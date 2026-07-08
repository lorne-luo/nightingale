use app_core::{AppConfig, LibrarySource, YoutubeSearchResult, download_youtube_video, search_youtube};

use crate::scanner::trigger_scan;

#[tauri::command]
pub async fn search_youtube_songs(query: String) -> Vec<YoutubeSearchResult> {
    tauri::async_runtime::spawn_blocking(move || search_youtube(&query, 8))
        .await
        .unwrap_or_default()
}

#[tauri::command]
pub async fn download_youtube_song(url: String) -> Result<(), String> {
    let Some(LibrarySource::Folder { path }) = AppConfig::load().library_source else {
        return Err(
            "Adding songs from YouTube requires a local folder library source.".to_string(),
        );
    };

    let dest_dir = path.join("YouTube Downloads");

    tauri::async_runtime::spawn_blocking(move || download_youtube_video(&url, &dest_dir))
        .await
        .map_err(|e| format!("Download task panicked: {e}"))??;

    trigger_scan();
    Ok(())
}
