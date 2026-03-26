# Groq Whisper 集成实施计划

## 目标

在 Nightingale 中增加基于 Groq Whisper 的转录路径，并遵循以下约束：

- 只要设置了 `GROQ_API_KEY`，就优先使用 Groq Whisper
- 未设置 `GROQ_API_KEY` 时，继续使用本地 WhisperX
- Groq 路径直接吃分离后的人声 stem
- 语言以 Groq 返回结果为准
- 播放层从逐字高亮改为逐句高亮
- 尽量缩小改动面，但允许为数据结构和 UI 行为做必要调整

## 当前问题

现有实现默认围绕 WhisperX 的逐词时间戳设计：

- Python 侧 `transcribe.py` 会输出包含 `words` 的 transcript
- Rust 侧 `Transcript` / `Segment` 结构默认要求 `words`
- 播放器按 `words[*].start/end` 做逐字高亮
- `server.py` / `pipeline.py` 还会预加载 WhisperX 模型

这意味着 Groq 集成不能只是新增一个 Python 文件并在 `transcribe.py` 里切换。否则即使设置了 `GROQ_API_KEY`，仍然会先加载本地 WhisperX，既浪费启动时间，也和“优先用 Groq”不一致。

## 目标架构

```text
Rust (analyzer/mod.rs)
  └─> 启动 Python 服务器 (server.py)
       └─> 调用 pipeline.py
            ├─> if GROQ_API_KEY exists
            │    └─> transcribe_groq.py
            │         └─> 调 Groq Whisper API，返回 segment 级 transcript
            └─> else
                 └─> transcribe.py
                      └─> 使用本地 WhisperX
```

关键点：

- 引擎选择前移到 `pipeline.py`
- 只有本地 WhisperX 路径才允许 preload model
- Groq transcript 只提供 segment 级时间戳
- 播放层不再依赖逐词时间戳

## 改动范围

### 新建文件

- `analyzer/transcribe_groq.py`
- `groq.md`

### 修改文件

- `analyzer/pipeline.py`
- `analyzer/server.py`
- `analyzer/requirements.txt`
- `src/vendor_scripts.rs`
- `src/analyzer/transcript.rs`
- `src/player/lyrics.rs`
- `src/player/mod.rs`

### 明确不做

- 不实现 Groq 失败后自动回退 WhisperX
- 不保留逐字高亮
- 不从 segment 时间戳估算伪造词级时间戳

## 实施步骤

### Phase 1: 新增 Groq 转录模块

文件：`analyzer/transcribe_groq.py`

职责：

- 读取分离后的人声音频文件
- 调用 Groq 音频转录接口
- 解析 `language` 和 `segments`
- 输出符合 Nightingale transcript 格式的结果

建议接口：

```python
def transcribe_with_groq(vocals_path: str) -> dict:
    """
    Returns:
        {
            "language": str,
            "segments": [
                {
                    "text": str,
                    "start": float,
                    "end": float,
                    "words": []
                }
            ],
            "source": "generated"
        }
    """
```

实现要求：

- 从 `GROQ_API_KEY` 读取凭证
- 直接上传 `vocals_path`
- 使用 Groq 返回的 `language`
- 每个 segment 统一带上 `words: []`，避免 Rust 侧反序列化失败
- 保留日志输出，方便区分当前走的是 Groq 路径

建议请求形态：

```python
client.audio.transcriptions.create(
    file=audio_file,
    model="whisper-large-v3-turbo",
    response_format="verbose_json",
)
```

注：

- 这里不传 `language_override`
- 这里不做本地语言检测
- 这里不做人为词级时间戳估算

### Phase 2: 在 pipeline 层做引擎选择

文件：`analyzer/pipeline.py`

调整目标：

- 在 `transcribe_or_align()` 中优先判断 `GROQ_API_KEY`
- 如果存在 key，则直接走 `transcribe_with_groq()`
- 如果不存在 key，则走现有 `transcribe_vocals()`

建议逻辑：

```python
if lyrics_path and os.path.isfile(lyrics_path):
    ...

if os.environ.get("GROQ_API_KEY"):
    return transcribe_with_groq(vocals_path)

return transcribe_vocals(...)
```

原因：

- 选择逻辑必须在 WhisperX preload 之前完成
- 这样才能真正做到“设置了 key 就优先用 Groq”

### Phase 3: 避免 Groq 路径预加载本地 WhisperX

文件：`analyzer/server.py`

当前问题：

- `process_song()` 会先构造 `whisper_model=lambda: _get_whisper(...)`
- `run_pipeline()` 当前会在转录前调用这个 lambda

需要改成：

- 只有本地 WhisperX 路径才去触发 `_get_whisper(...)`
- Groq 路径下不加载 WhisperX

可选做法：

1. 在 `pipeline.py` 内部先决定引擎，再按需调用 `whisper_model()`
2. 或者在 `server.py` 中先判断环境变量，只在无 key 时传 preload 回调

推荐做法：

- 把按需加载收敛在 `pipeline.py`

原因：

- 引擎选择逻辑集中
- `server.py` 不需要知道太多具体转录实现

### Phase 4: 更新 Python 依赖和脚本打包

文件：

- `analyzer/requirements.txt`
- `src/vendor_scripts.rs`

需要做的事：

- 在 `requirements.txt` 中加入 `groq`
- 在 `vendor_scripts.rs` 中加入 `transcribe_groq.py`

如果不改这里：

- vendored Python 环境运行时会缺少 `groq`
- analyzer 临时目录里也不会写出新脚本

### Phase 5: Transcript 结构兼容 segment-only 模式

文件：`src/analyzer/transcript.rs`

当前问题：

- `Segment.words` 是必填
- 现有代码默认 transcript 里总是有词级信息

建议改法：

- 保持 `words: Vec<Word>` 不变
- 给 `Segment.words` 增加 `#[serde(default)]`

目标：

- Groq transcript 可以合法写入 `words: []`
- 本地 WhisperX transcript 仍然继续携带完整词级信息

建议结构：

```rust
pub struct Segment {
    pub text: String,
    pub start: f64,
    pub end: f64,
    #[serde(default)]
    pub words: Vec<Word>,
}
```

### Phase 6: 播放器从逐字高亮改为逐句高亮

文件：

- `src/player/lyrics.rs`
- `src/player/mod.rs`

改造目标：

- 当前行按 segment 的 `start/end` 高亮
- 不再依赖 `words[*].start/end`
- `words` 为空时也能正常展示歌词

具体方向：

- 保留基于 segment 的当前句、下一句、倒计时逻辑
- 删除或绕过逐词颜色渐变逻辑
- 句子激活条件只使用 `seg.start` 和 `seg.end`
- 行内文本渲染改成直接渲染 `seg.text`

实现建议：

1. 检查 `setup_lyrics()` 和 `rebuild_lines()` 中是否默认按 `words` 构建文本节点
2. 改成优先使用 `seg.text`
3. 若 `seg.words` 非空，也不再做逐词着色
4. 将所有词级高亮逻辑收口为 segment 级显示状态

预期结果：

- WhisperX 路径可继续携带 `words`，但 UI 不再依赖它
- Groq 路径即使 `words` 为空，播放器也能正常工作

### Phase 7: 保持本地 WhisperX 路径不变

文件：`analyzer/transcribe.py`

目标：

- 尽量不动现有 WhisperX 逻辑
- 继续保留当前转录、过滤、对齐能力

注意：

- 既然引擎选择已前移，`transcribe.py` 不需要再感知 Groq
- 这能降低对现有本地路径的回归风险

## 验证方案

### 场景 1: 无 `GROQ_API_KEY`

预期：

- 走本地 WhisperX
- transcript 正常生成
- 播放时逐句高亮正常

### 场景 2: 有 `GROQ_API_KEY`

预期：

- 不 preload WhisperX
- 直接调用 Groq
- transcript 里 `segments` 有时间戳，`words` 为空数组
- 播放时逐句高亮正常

### 场景 3: 已有歌词文件

预期：

- 继续优先走歌词对齐逻辑
- 不受 Groq 路径影响

### 场景 4: Transcript 兼容性

预期：

- 旧 transcript 文件仍能加载
- 新 Groq transcript 文件能被 Rust 正常反序列化

## 风险与处理

### 风险 1: Groq 返回的 segment 切分过长

影响：

- 某些句子可能过长，显示体验不稳定

处理：

- 先接受 Groq 的原始 segment
- 如显示效果不佳，再单独增加基于标点或长度的 segment 拆分逻辑

### 风险 2: UI 从逐字切到逐句后，WhisperX 的词级数据价值下降

影响：

- 现有词级精度不再直接体现在播放层

处理：

- 这是明确的产品决策，不视为回归
- 词级数据仍可保留，为后续功能预留

### 风险 3: Groq API 调用失败

影响：

- 设置了 key 的情况下，本次分析失败

处理：

- 直接报错
- 不做自动回退，符合当前需求

## 最终验收标准

- 设置 `GROQ_API_KEY` 时，分析流程优先走 Groq
- Groq 路径下不再加载本地 WhisperX 模型
- 未设置 `GROQ_API_KEY` 时，本地 WhisperX 行为保持可用
- 播放器改为逐句高亮
- Groq transcript 使用 segment 级时间戳即可正常播放
- vendored analyzer 环境包含 `transcribe_groq.py` 和 `groq` 依赖
