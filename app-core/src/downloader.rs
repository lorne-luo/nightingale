fn is_youtube_host(host: &str) -> bool {
    let host = host.strip_prefix("www.").unwrap_or(host);
    matches!(
        host,
        "youtube.com" | "m.youtube.com" | "music.youtube.com" | "youtu.be"
    )
}

fn youtube_short_host(host: &str) -> bool {
    host.strip_prefix("www.").unwrap_or(host) == "youtu.be"
}

fn valid_video_id(id: &str) -> Option<String> {
    if id.len() == 11
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
    {
        Some(id.to_string())
    } else {
        None
    }
}

fn extract_video_id_from_url(parsed: &url::Url) -> Option<String> {
    let host = parsed.host_str()?;
    if !is_youtube_host(host) {
        return None;
    }

    if youtube_short_host(host) {
        let id = parsed.path().trim_start_matches('/');
        let id = id.split(&['?', '#', '/'][..]).next()?;
        return valid_video_id(id);
    }

    if parsed.path() != "/watch" {
        return None;
    }

    let v = parsed
        .query_pairs()
        .find(|(key, _)| key == "v")
        .map(|(_, value)| value.into_owned())?;
    valid_video_id(&v)
}

/// Validates a YouTube URL using proper parsing.
pub fn is_valid_youtube_url(url: &str) -> bool {
    let parsed = match url::Url::parse(url.trim()) {
        Ok(url) => url,
        Err(_) => return false,
    };

    if parsed.scheme() != "https" {
        return false;
    }

    extract_video_id_from_url(&parsed).is_some()
}

/// Extracts the YouTube video ID from a URL.
pub fn extract_video_id(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url.trim()).ok()?;
    extract_video_id_from_url(&parsed)
}

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    cache::CacheDir,
    vendor::{ensure_yt_dlp_ready, ffmpeg_path, silent_command, yt_dlp_path},
};

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

/// Searches YouTube via `yt-dlp`'s built-in `ytsearchN:` support.
pub fn search_youtube(query: &str, limit: usize) -> Result<Vec<YoutubeSearchResult>, String> {
    ensure_yt_dlp_ready()?;

    let yt_dlp = yt_dlp_path();
    let search_spec = format!("ytsearch{limit}:{query}");

    let output = silent_command(&yt_dlp)
        .args([
            "--dump-json",
            "--flat-playlist",
            "--no-warnings",
            "--socket-timeout",
            "15",
            "--retries",
            "2",
            &search_spec,
        ])
        .output()
        .map_err(|e| format!("Failed to run yt-dlp search: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("yt-dlp search failed: {}", stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let non_empty_lines: Vec<_> = stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    let results: Vec<_> = non_empty_lines
        .iter()
        .filter_map(|line| serde_json::from_str::<YoutubeSearchResult>(line).ok())
        .collect();

    if !non_empty_lines.is_empty() && results.is_empty() {
        return Err("yt-dlp returned search results in an unsupported format".to_string());
    }

    Ok(results)
}

use std::path::{Path, PathBuf};

/// Downloads a YouTube video (1080p max, merged with best audio into mp4)
/// into `dest_dir`, creating it if needed. Returns the final file path.
pub fn download_youtube_video(url: &str, dest_dir: &Path) -> Result<PathBuf, String> {
    if !is_valid_youtube_url(url) {
        return Err(format!("Not a valid YouTube URL: {url}"));
    }
    let video_id = extract_video_id(url).expect("validated YouTube URL must contain a video ID");
    ensure_yt_dlp_ready()?;

    std::fs::create_dir_all(dest_dir).map_err(|e| format!("Failed to create directory: {e}"))?;

    let cache = CacheDir::new();
    let temp_dir = tempfile::Builder::new()
        .prefix("youtube-download-")
        .tempdir_in(&cache.path)
        .map_err(|e| format!("Failed to create temporary download directory: {e}"))?;
    let output_template = temp_dir.path().join("%(title)s.%(ext)s");
    let output_str = output_template.to_string_lossy().into_owned();
    let manifest = temp_dir.path().join("download-path.txt");
    let manifest_str = manifest.to_string_lossy().into_owned();
    let ffmpeg = ffmpeg_path().to_string_lossy().into_owned();

    let output = silent_command(yt_dlp_path())
        .args([
            "--ffmpeg-location",
            &ffmpeg,
            "-f",
            "bestvideo[height<=1080]+bestaudio/best",
            "--merge-output-format",
            "mp4",
            "--restrict-filenames",
            "--no-playlist",
            "--no-warnings",
            "--socket-timeout",
            "30",
            "--retries",
            "3",
            "--fragment-retries",
            "3",
            "-o",
            &output_str,
            "--print-to-file",
            "after_move:filepath",
            &manifest_str,
            url,
        ])
        .output()
        .map_err(|e| format!("Failed to start yt-dlp: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("yt-dlp download failed: {stderr}"));
    }

    let manifest_contents = std::fs::read_to_string(&manifest)
        .map_err(|e| format!("Download completed without an output manifest: {e}"))?;
    let downloaded_path = manifest_contents
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| "Download completed without an output path".to_string())?;

    let temp_root = temp_dir
        .path()
        .canonicalize()
        .map_err(|e| format!("Failed to resolve temporary directory: {e}"))?;
    let downloaded_path = downloaded_path
        .canonicalize()
        .map_err(|e| format!("Downloaded file was not found: {e}"))?;
    if !downloaded_path.starts_with(&temp_root) {
        return Err("yt-dlp returned a file outside the temporary directory".to_string());
    }

    let file_name = downloaded_path
        .file_name()
        .ok_or_else(|| "Downloaded file has no filename".to_string())?;
    let video_dir = dest_dir.join(video_id);
    std::fs::create_dir_all(&video_dir)
        .map_err(|e| format!("Failed to create video directory: {e}"))?;
    let final_path = video_dir.join(file_name);

    install_download(&downloaded_path, &final_path)
}

fn install_download(source: &Path, destination: &Path) -> Result<PathBuf, String> {
    if destination.is_file() {
        return Ok(destination.to_path_buf());
    }

    let extension = destination
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("media");
    let partial = destination.with_extension(format!("{extension}.part-{}", rand::random::<u64>()));

    if let Err(e) = std::fs::copy(source, &partial) {
        let _ = std::fs::remove_file(&partial);
        return Err(format!("Failed to copy downloaded file into library: {e}"));
    }

    match std::fs::rename(&partial, destination) {
        Ok(()) => {
            let _ = std::fs::remove_file(source);
            Ok(destination.to_path_buf())
        }
        Err(_) if destination.is_file() => {
            let _ = std::fs::remove_file(&partial);
            Ok(destination.to_path_buf())
        }
        Err(e) => {
            let _ = std::fs::remove_file(&partial);
            Err(format!("Failed to finalize downloaded file: {e}"))
        }
    }
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
    fn rejects_youtube_substring_on_other_host() {
        assert!(!is_valid_youtube_url(
            "https://evil.com/path/youtube.com/watch?v=dQw4w9WgXcQ"
        ));
    }

    #[test]
    fn accepts_v_param_not_first_in_query() {
        assert!(is_valid_youtube_url(
            "https://www.youtube.com/watch?feature=share&v=dQw4w9WgXcQ"
        ));
    }

    #[test]
    fn accepts_url_with_fragment() {
        assert!(is_valid_youtube_url(
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ#t=42"
        ));
    }

    #[test]
    fn extracts_id_from_url_with_fragment() {
        assert_eq!(
            extract_video_id("https://www.youtube.com/watch?v=dQw4w9WgXcQ#t=42").as_deref(),
            Some("dQw4w9WgXcQ")
        );
    }

    #[test]
    fn rejects_url_without_scheme() {
        assert!(!is_valid_youtube_url("www.youtube.com/watch?v=dQw4w9WgXcQ"));
    }

    #[test]
    fn rejects_http_url() {
        assert!(!is_valid_youtube_url(
            "http://www.youtube.com/watch?v=dQw4w9WgXcQ"
        ));
    }

    #[test]
    fn rejects_invalid_video_id_characters() {
        assert!(!is_valid_youtube_url(
            "https://www.youtube.com/watch?v=dQw4w9WgXc!"
        ));
    }

    #[test]
    fn rejects_video_id_with_wrong_length() {
        assert!(!is_valid_youtube_url(
            "https://www.youtube.com/watch?v=dQw4w9WgXcQQ"
        ));
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

    #[test]
    fn installs_download_without_overwriting_existing_file() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source.mp4");
        let destination = temp.path().join("destination.mp4");
        std::fs::write(&source, b"new").unwrap();
        std::fs::write(&destination, b"existing").unwrap();

        assert_eq!(
            install_download(&source, &destination).unwrap(),
            destination
        );
        assert_eq!(std::fs::read(&destination).unwrap(), b"existing");
    }

    #[test]
    fn installs_download_at_destination() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source.mp4");
        let destination = temp.path().join("destination.mp4");
        std::fs::write(&source, b"video").unwrap();

        assert_eq!(
            install_download(&source, &destination).unwrap(),
            destination
        );
        assert_eq!(std::fs::read(&destination).unwrap(), b"video");
        assert!(!source.exists());
    }
}
