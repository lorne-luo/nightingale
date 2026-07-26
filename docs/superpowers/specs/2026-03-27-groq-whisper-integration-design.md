# Groq Whisper 集成设计

## 概述

在 Nightingale 中集成 Groq Whisper API 作为云端转录路径，与现有本地 WhisperX 形成二级架构。设置 `GROQ_API_KEY` 时优先使用 Groq，否则回退到本地 WhisperX。

## 目标约束

1. 优先级：`GROQ_API_KEY` 存在 → Groq Whisper；否则 → 本地 WhisperX
2. Groq 路径直接使用分离后的人声 stem
3. 语言以 Groq 返回结果为准
4. 播放层从逐字高亮改为逐句高亮
5. 最小化改动，必要时调整数据结构和 UI 行为

## 当前架构

```
Rust (analyzer/mod.rs)
  └─> 启动 Python 服务器 (server.py)
       └─> 调用 pipeline.py
            └─> transcribe.py (WhisperX)
                 └─> 返回 word-level transcript
```

## 目标架构

```
Rust (analyzer/mod.rs)
  └─> 启动 Python 服务器 (server.py)
       └─> 调用 pipeline.py
            ├─> if GROQ_API_KEY exists
            │    └─> transcribe_groq.py
            │         └─> Groq Whisper API (segment-level)
            └─> else
                 └─> transcribe.py
                      └─> 本地 WhisperX (word-level)
```

## 关键设计决策

### 决策 1: 引擎选择前移

引擎选择逻辑从转录层前移到 pipeline 层，确保在 WhisperX 模型加载之前就做出决策。

**原因**：避免在 Groq 路径下不必要地加载本地 WhisperX 模型。

### 决策 2: Segment-only 兼容

Rust 侧的 `Segment.words` 字段从必填改为可选，使用 `#[serde(default)]`。

**原因**：Groq API 只返回 segment 级时间戳，不提供 word 级。

### 决策 3: 逐句高亮

播放器不再依赖 `words[*].start/end`，改为使用 `segment.start/end` 进行高亮。

**原因**：
- Groq 不提供 word 级时间戳
- 避免伪造词级时间戳
- 简化播放逻辑

## 改动范围

### 新建文件

| 文件 | 职责 |
|------|------|
| `app-core/analyzer/transcribe_groq.py` | Groq Whisper API 转录模块 |
| `groq.md` | 用户文档（可选） |

### 修改文件

| 文件 | 改动内容 |
|------|----------|
| `app-core/analyzer/pipeline.py` | 添加引擎选择逻辑 |
| `app-core/analyzer/server.py` | 避免 Groq 路径的 WhisperX 预加载 |
| `app-core/analyzer/requirements.txt` | 添加 `groq` 依赖 |
| `src/vendor_scripts.rs` | 包含 `transcribe_groq.py` |
| `src/analyzer/transcript.rs` | `Segment.words` 添加 `#[serde(default)]` |
| `src/player/lyrics.rs` | 改为逐句高亮 |
| `src/player/mod.rs` | 播放器适配 |

## 实施计划

详见 `groq2.md` 中的 7 个阶段：

1. **Phase 1**: 新增 Groq 转录模块
2. **Phase 2**: pipeline 层引擎选择
3. **Phase 3**: 避免 Groq 路径预加载 WhisperX
4. **Phase 4**: 更新依赖和脚本打包
5. **Phase 5**: Transcript 结构兼容
6. **Phase 6**: 播放器逐句高亮
7. **Phase 7**: 保持本地路径不变

## 验收标准

- [ ] 设置 `GROQ_API_KEY` 时优先使用 Groq
- [ ] Groq 路径不加载本地 WhisperX 模型
- [ ] 未设置 key 时本地 WhisperX 行为正常
- [ ] 播放器逐句高亮正常工作
- [ ] Groq transcript 可正常播放（segment 级时间戳）
- [ ] Vendored 环境包含新依赖和脚本

## 风险处理

| 风险 | 处理 |
|------|------|
| Groq segment 过长 | 先接受原始切分，后续优化 |
| Word 级数据价值下降 | 产品决策，不视为回归 |
| API 调用失败 | 直接报错，不做自动回退 |