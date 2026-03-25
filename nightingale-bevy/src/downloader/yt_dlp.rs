use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use super::{DownloadError, DownloadResult, DownloadStatus, VideoDownloader, lock_mutex_safe};

/// yt-dlp based video downloader implementation
pub struct YtDlpDownloader {
    path: PathBuf,
}

impl YtDlpDownloader {
    pub fn new() -> Self {
        let path = crate::vendor::venv_bin_dir().join(if cfg!(windows) {
            "yt-dlp.exe"
        } else {
            "yt-dlp"
        });
        Self { path }
    }

    /// Returns the path to the yt-dlp binary
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Default for YtDlpDownloader {
    fn default() -> Self {
        Self::new()
    }
}

impl VideoDownloader for YtDlpDownloader {
    fn name(&self) -> &str {
        "yt-dlp"
    }

    fn is_available(&self) -> bool {
        self.path.is_file()
    }

    fn download(&self, url: &str, dest_dir: &std::path::Path) -> DownloadResult {
        let progress = Arc::new(Mutex::new(DownloadStatus::Starting));
        Self::do_download(&self.path, url, dest_dir, &progress)
    }

    fn download_async(
        &self,
        url: String,
        dest_dir: PathBuf,
        progress: Arc<Mutex<DownloadStatus>>,
    ) -> JoinHandle<DownloadResult> {
        let yt_dlp_path = self.path.clone();

        thread::spawn(move || Self::do_download(&yt_dlp_path, &url, &dest_dir, &progress))
    }
}

impl YtDlpDownloader {
    fn do_download(
        yt_dlp_path: &std::path::Path,
        url: &str,
        dest_dir: &std::path::Path,
        progress: &Arc<Mutex<DownloadStatus>>,
    ) -> DownloadResult {
        // Ensure destination directory exists
        if let Err(e) = std::fs::create_dir_all(dest_dir) {
            let err = DownloadError::DownloadFailed(format!("Failed to create directory: {}", e));
            *lock_mutex_safe(progress) = DownloadStatus::Failed(err.to_string());
            return Err(err);
        }

        // Update status: starting
        *lock_mutex_safe(progress) = DownloadStatus::Starting;

        // Build yt-dlp command
        // -f "bestvideo[height<=1080]+bestaudio/best" : download 1080p video + best audio, with fallback
        // --merge-output-format mp4 : merge to mp4
        // -o "%(title)s.%(ext)s" : output filename
        // --print after_move:filepath : print the final file path after download
        let output_template = dest_dir.join("%(title)s.%(ext)s");
        let output_str = output_template.to_string_lossy();

        let args: Vec<String> = vec![
            "-f".into(),
            "bestvideo[height<=1080]+bestaudio/best".into(),
            "--merge-output-format".into(),
            "mp4".into(),
            "-o".into(),
            output_str.into_owned(),
            "--no-playlist".into(),
            "--no-warnings".into(),
            "--print".into(),
            "after_move:filepath".into(),
            url.into(),
        ];

        let result = crate::vendor::silent_command(yt_dlp_path)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();

        let mut child = match result {
            Ok(c) => c,
            Err(e) => {
                let err = DownloadError::DownloadFailed(format!("Failed to start yt-dlp: {}", e));
                *lock_mutex_safe(progress) = DownloadStatus::Failed(err.to_string());
                return Err(err);
            }
        };

        // Collect stdout to get the final file path
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let stdout_result = if let Some(stdout) = stdout {
            let reader = BufReader::new(stdout);
            let mut lines = Vec::new();
            for line in reader.lines().flatten() {
                lines.push(line);
            }
            Some(lines)
        } else {
            None
        };

        // Collect stderr for error messages
        let stderr_result = if let Some(mut stderr) = stderr {
            let mut buf = String::new();
            let _ = stderr.read_to_string(&mut buf);
            Some(buf)
        } else {
            None
        };

        // Wait for completion
        let status = match child.wait() {
            Ok(s) => s,
            Err(e) => {
                let err =
                    DownloadError::DownloadFailed(format!("Failed to wait for yt-dlp: {}", e));
                *lock_mutex_safe(progress) = DownloadStatus::Failed(err.to_string());
                return Err(err);
            }
        };

        if !status.success() {
            let error_msg = stderr_result
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| "Unknown error".into());

            let err = DownloadError::DownloadFailed(error_msg);
            *lock_mutex_safe(progress) = DownloadStatus::Failed(err.to_string());
            return Err(err);
        }

        // Find the downloaded file path from stdout
        // yt-dlp with --print after_move:filepath prints the final path
        if let Some(ref lines) = stdout_result {
            // The last line should be the file path
            for line in lines.iter().rev() {
                let path = PathBuf::from(line.trim());
                if path.exists() && path.extension().is_some_and(|ext| ext == "mp4") {
                    *lock_mutex_safe(progress) = DownloadStatus::Completed {
                        video_path: path.clone(),
                    };
                    return Ok(path);
                }
            }
        }

        // Fallback: find the most recently modified mp4 file in the directory
        // This handles edge cases where --print might not work as expected
        if let Ok(entries) = std::fs::read_dir(dest_dir) {
            let mut mp4_files: Vec<PathBuf> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|ext| ext == "mp4"))
                .collect();

            // Sort by modification time, most recent first
            mp4_files.sort_by(|a, b| {
                let a_time = a.metadata().and_then(|m| m.modified()).ok();
                let b_time = b.metadata().and_then(|m| m.modified()).ok();
                b_time.cmp(&a_time)
            });

            if let Some(video_path) = mp4_files.into_iter().next() {
                *lock_mutex_safe(progress) = DownloadStatus::Completed {
                    video_path: video_path.clone(),
                };
                return Ok(video_path);
            }
        }

        let err = DownloadError::DownloadFailed("Download completed but no file found".into());
        *lock_mutex_safe(progress) = DownloadStatus::Failed(err.to_string());
        Err(err)
    }
}
