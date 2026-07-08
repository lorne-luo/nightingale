/// Validates a YouTube URL using proper parsing.
pub fn is_valid_youtube_url(url: &str) -> bool {
    let url = url.trim();

    if !url.starts_with("http://") && !url.starts_with("https://") {
        return false;
    }

    let lower = url.to_lowercase();
    let is_youtube = lower.contains("youtube.com/watch")
        || lower.contains("www.youtube.com/watch")
        || lower.contains("m.youtube.com/watch")
        || lower.contains("youtu.be/");

    if !is_youtube {
        return false;
    }

    extract_video_id(url).is_some()
}

/// Extracts the YouTube video ID from a URL.
pub fn extract_video_id(url: &str) -> Option<String> {
    let url = url.trim();

    if url.contains("youtu.be/") {
        let start = url.find("youtu.be/")? + 9;
        let rest = &url[start..];
        let end = rest
            .find('?')
            .or_else(|| rest.find('&'))
            .or_else(|| rest.find('/'))
            .unwrap_or(rest.len());
        let id = &rest[..end];
        if id.len() >= 11 && id.len() <= 12 {
            return Some(id.to_string());
        }
        return None;
    }

    if let Some(pos) = url.find("v=") {
        let start = pos + 2;
        let rest = &url[start..];
        let end = rest.find('&').unwrap_or(rest.len());
        let id = &rest[..end];
        if id.len() >= 11 && id.len() <= 12 {
            return Some(id.to_string());
        }
    }

    None
}

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::vendor::{silent_command, yt_dlp_path};

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct YoutubeSearchResult {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub channel: String,
    #[serde(default, alias = "duration")]
    pub duration_secs: f64,
}

/// Searches YouTube via `yt-dlp`'s built-in `ytsearchN:` support. No API key
/// needed. Returns an empty list on any failure (matches `lrclib_candidates`'s
/// swallow-and-log-warn behavior for search — a search "error" and "no
/// results" render identically in the UI, same as the LRCLIB search).
pub fn search_youtube(query: &str, limit: usize) -> Vec<YoutubeSearchResult> {
    let yt_dlp = yt_dlp_path();
    let search_spec = format!("ytsearch{limit}:{query}");

    let output = match silent_command(&yt_dlp)
        .args(["--dump-json", "--flat-playlist", "--no-warnings", &search_spec])
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            tracing::warn!("[downloader] Failed to run yt-dlp search: {e}");
            return Vec::new();
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::warn!("[downloader] yt-dlp search failed: {stderr}");
        return Vec::new();
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str::<YoutubeSearchResult>(line).ok())
        .collect()
}

use std::path::{Path, PathBuf};

/// Downloads a YouTube video (1080p max, merged with best audio into mp4)
/// into `dest_dir`, creating it if needed. Returns the final file path.
pub fn download_youtube_video(url: &str, dest_dir: &Path) -> Result<PathBuf, String> {
    if !is_valid_youtube_url(url) {
        return Err(format!("Not a valid YouTube URL: {url}"));
    }

    std::fs::create_dir_all(dest_dir)
        .map_err(|e| format!("Failed to create directory: {e}"))?;

    let output_template = dest_dir.join("%(title)s.%(ext)s");
    let output_str = output_template.to_string_lossy().into_owned();

    let output = silent_command(yt_dlp_path())
        .args([
            "-f",
            "bestvideo[height<=1080]+bestaudio/best",
            "--merge-output-format",
            "mp4",
            "--restrict-filenames",
            "--no-playlist",
            "--no-warnings",
            "-o",
            &output_str,
            "--print",
            "after_move:filepath",
            url,
        ])
        .output()
        .map_err(|e| format!("Failed to start yt-dlp: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("yt-dlp download failed: {stderr}"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines().rev() {
        let path = PathBuf::from(line.trim());
        if path.is_file() {
            return Ok(path);
        }
    }

    // Fallback: `--print` output wasn't a usable path — pick the most
    // recently modified mp4 in dest_dir.
    let mut mp4_files: Vec<PathBuf> = std::fs::read_dir(dest_dir)
        .map_err(|e| format!("Failed to read destination directory: {e}"))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "mp4"))
        .collect();

    mp4_files.sort_by_key(|p| {
        p.metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
    });

    mp4_files
        .pop()
        .ok_or_else(|| "Download completed but no file found".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_standard_watch_url() {
        assert!(is_valid_youtube_url(
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
        ));
    }

    #[test]
    fn accepts_youtu_be_short_url() {
        assert!(is_valid_youtube_url("https://youtu.be/dQw4w9WgXcQ"));
    }

    #[test]
    fn accepts_url_with_extra_query_params() {
        assert!(is_valid_youtube_url(
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=42s"
        ));
    }

    #[test]
    fn rejects_non_youtube_domain() {
        assert!(!is_valid_youtube_url("https://vimeo.com/12345678"));
    }

    #[test]
    fn rejects_url_without_scheme() {
        assert!(!is_valid_youtube_url("www.youtube.com/watch?v=dQw4w9WgXcQ"));
    }

    #[test]
    fn rejects_youtube_url_without_video_id() {
        assert!(!is_valid_youtube_url("https://www.youtube.com/watch"));
    }

    #[test]
    fn extracts_id_from_standard_url() {
        assert_eq!(
            extract_video_id("https://www.youtube.com/watch?v=dQw4w9WgXcQ").as_deref(),
            Some("dQw4w9WgXcQ")
        );
    }

    #[test]
    fn extracts_id_from_short_url() {
        assert_eq!(
            extract_video_id("https://youtu.be/dQw4w9WgXcQ").as_deref(),
            Some("dQw4w9WgXcQ")
        );
    }

    #[test]
    fn extracts_id_ignoring_trailing_query_params() {
        assert_eq!(
            extract_video_id("https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=42s").as_deref(),
            Some("dQw4w9WgXcQ")
        );
    }

    #[test]
    fn returns_none_for_missing_id() {
        assert_eq!(extract_video_id("https://www.youtube.com/watch"), None);
    }
}
