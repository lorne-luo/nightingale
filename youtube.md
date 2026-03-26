# YouTube 视频下载功能设计

## 概述

添加从 YouTube 下载视频的功能，用户可以通过侧边栏的 "YouTube Link" 按钮输入链接，下载的视频将存入用户选择的音乐文件夹，并自动加入转录分析队列。

## 需求总结

| 项目 | 决策 |
|-----|------|
| yt-dlp 安装方式 | pip 自动安装到 venv |
| 视频存储位置 | 用户选择的音乐文件夹 |
| UI 按钮位置 | Rescan Folder 按钮下方 |
| 下载格式 | 1080p 视频+最佳音频合并为 mp4 |
| 链接输入方式 | 弹出对话框，包含输入框和确认/取消按钮 |

## 架构设计

```
┌─────────────────────────────────────────────────────────┐
│                      Sidebar                             │
│  ┌─────────────────────┐                                │
│  │ Select Folder       │                                │
│  │ Rescan Folder       │                                │
│  │ YouTube Link ←──────┼── 点击弹出对话框                │
│  └─────────────────────┘                                │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│                  YouTube Dialog                          │
│  ┌─────────────────────────────────────────────────────┐│
│  │  [输入 YouTube 链接________________]                ││
│  │                                                     ││
│  │  [Cancel]              [Download]                   ││
│  └─────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│              VideoDownloader (抽象层)                    │
│  ┌─────────────────────────────────────────────────┐   │
│  │  trait VideoDownloader                           │   │
│  │    - download(url, dest_path) -> Result<Path>    │   │
│  │    - name() -> &str                              │   │
│  │    - is_available() -> bool                      │   │
│  └─────────────────────────────────────────────────┘   │
│       │                              │                  │
│       ▼                              ▼                  │
│  ┌──────────────┐           ┌──────────────┐           │
│  │  YtDlp       │           │  (其他实现)   │           │
│  │  Downloader  │           │  gallery-dl  │           │
│  └──────────────┘           └──────────────┘           │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│                  SongLibrary + AnalysisQueue             │
│  - 下载完成后创建 Song 并加入 library                    │
│  - 自动加入 AnalysisQueue 进行转录分析                  │
└─────────────────────────────────────────────────────────┘
```

## 模块结构

新增 `src/downloader/` 目录：

```
src/
├── downloader/
│   ├── mod.rs           # 公开接口 + VideoDownloader trait
│   ├── yt_dlp.rs        # yt-dlp 具体实现
│   └── dialog.rs        # YouTube 链接对话框 UI
├── menu/
│   └── sidebar.rs       # 添加 YouTube Link 按钮
└── vendor.rs            # 添加 yt-dlp pip 安装
```

## 核心接口设计

### VideoDownloader Trait

```rust
// src/downloader/mod.rs

pub type DownloadResult = Result<PathBuf, DownloadError>;

#[derive(Debug)]
pub enum DownloadError {
    NotInstalled,
    InvalidUrl(String),
    DownloadFailed(String),
    NetworkError(String),
}

pub trait VideoDownloader: Send + Sync {
    /// 下载器名称（用于日志和 UI 显示）
    fn name(&self) -> &str;

    /// 检查下载器是否可用
    fn is_available(&self) -> bool;

    /// 下载视频到指定目录
    fn download(&self, url: &str, dest_dir: &Path) -> DownloadResult;

    /// 异步下载（后台线程调用）
    fn download_async(
        &self,
        url: String,
        dest_dir: PathBuf,
    ) -> std::thread::JoinHandle<DownloadResult>;
}

/// 当前使用的下载器（可配置）
pub fn get_downloader() -> Box<dyn VideoDownloader> {
    // 目前返回 yt-dlp，未来可改为配置驱动
    Box::new(YtDlpDownloader::new())
}
```

### YtDlpDownloader 实现

```rust
// src/downloader/yt_dlp.rs

pub struct YtDlpDownloader {
    path: PathBuf,
}

impl YtDlpDownloader {
    pub fn new() -> Self {
        let path = crate::vendor::venv_bin_dir().join("yt-dlp");
        Self { path }
    }
}

impl VideoDownloader for YtDlpDownloader {
    fn name(&self) -> &str { "yt-dlp" }

    fn is_available(&self) -> bool {
        self.path.is_file()
    }

    fn download(&self, url: &str, dest_dir: &Path) -> DownloadResult {
        // 调用 yt-dlp 下载 1080p 视频+最佳音频合并为 mp4
        // 命令: yt-dlp -f "bestvideo[height<=1080]+bestaudio" --merge-output-format mp4 -o "{dest_dir}/%(title)s.%(ext)s" {url}
    }

    fn download_async(&self, url: String, dest_dir: PathBuf) -> JoinHandle<DownloadResult> {
        let path = self.path.clone();
        std::thread::spawn(move || {
            // 后台线程执行下载
        })
    }
}
```

## 安装 yt-dlp

在 `vendor.rs` 的 `step_install_packages` 中添加 yt-dlp：

```rust
// 在包安装列表中添加 yt-dlp
let pkg_args: Vec<&str> = vec![
    "pip", "install",
    "demucs>=4.0.0", "whisperx>=3.3.0", "soundfile",
    "huggingface_hub>=0.27.0",
    audio_sep_pkg,
    "yt-dlp",  // 新增
    "--python", &py_str,
];
```

## UI 组件

### YouTube Link 按钮

- 位置：Rescan Folder 按钮下方
- 样式：复用 `spawn_sidebar_button` 函数
- 点击行为：弹出 YouTube 链接对话框

### YouTube 对话框

- 复用现有弹窗模式（类似 Settings、Profile）
- 组件：
  - 标题："Download from YouTube"
  - 输入框：用于粘贴 YouTube 链接
  - 按钮：Cancel / Download
- 下载中状态：显示进度指示器

## 数据流

```
用户点击 YouTube Link
        │
        ▼
弹出对话框，等待输入
        │
        ▼
用户输入链接，点击 Download
        │
        ▼
后台线程启动 yt-dlp 下载
        │
        ▼
下载完成，获取文件路径
        │
        ▼
创建 Song 对象（is_video = true）
        │
        ▼
添加到 SongLibrary
        │
        ▼
自动加入 AnalysisQueue
```

## 关键集成点

| 现有模块 | 集成方式 |
|---------|---------|
| `SongLibrary` | 下载完成后添加新 Song |
| `AnalysisQueue` | 自动入队分析 |
| `spawn_source_video_background` | 播放时直接使用 |
| `sidebar.rs` | 添加按钮，复用弹窗模式 |
| `vendor.rs` | pip 安装 yt-dlp |

## 文件清单

### 新增文件

| 文件 | 说明 |
|-----|------|
| `src/downloader/mod.rs` | 模块入口，VideoDownloader trait |
| `src/downloader/yt_dlp.rs` | yt-dlp 实现 |
| `src/downloader/dialog.rs` | YouTube 对话框 UI |

### 修改文件

| 文件 | 改动 |
|-----|------|
| `src/main.rs` | 添加 `mod downloader` |
| `src/vendor.rs` | 添加 yt-dlp pip 安装，添加 `venv_bin_dir()` 函数 |
| `src/menu/sidebar.rs` | 添加 YouTube Link 按钮和点击处理 |
| `src/menu/components.rs` | 添加 `YoutubeLinkButton` 组件 |

## 错误处理

| 错误场景 | 处理方式 |
|---------|---------|
| yt-dlp 未安装 | 按钮禁用或显示安装提示 |
| 无效 URL | 对话框显示错误信息 |
| 下载失败 | 对话框显示错误信息，允许重试 |
| 网络错误 | 对话框显示错误信息，允许重试 |

## 可扩展性设计

为了方便替换下载器：

1. **Trait 抽象**：所有下载器实现 `VideoDownloader` trait
2. **工厂函数**：`get_downloader()` 返回当前配置的下载器
3. **配置驱动**（未来）：可通过配置文件切换下载器实现

添加新下载器只需：
1. 创建新的实现文件（如 `gallery_dl.rs`）
2. 实现 `VideoDownloader` trait
3. 在 `get_downloader()` 中返回新实现