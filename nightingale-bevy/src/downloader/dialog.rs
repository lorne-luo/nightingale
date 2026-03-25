use std::sync::{Arc, Mutex};

use bevy::prelude::*;

use super::{
    ActiveDownload, DownloadStatus, YoutubeCancelButton, YoutubeDialogRoot, YoutubeDialogState,
    YoutubeDownloadButton, YoutubeErrorText, build_song_from_download, despawn_youtube_dialog,
    get_downloader, is_valid_youtube_url, lock_mutex_safe,
};
use crate::analyzer::AnalysisQueue;
use crate::analyzer::cache::CacheDir;
use crate::config::AppConfig;
use crate::scanner::metadata::{AnalysisStatus, SongLibrary};
use crate::ui::UiTheme;

pub fn handle_youtube_dialog(
    mut commands: Commands,
    config: Res<AppConfig>,
    mut dialog_state: ResMut<YoutubeDialogState>,
    mut cancel_query: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<YoutubeCancelButton>),
    >,
    mut download_query: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<YoutubeDownloadButton>),
    >,
    dialog_query: Query<Entity, With<YoutubeDialogRoot>>,
    mut error_text_query: Query<&mut Visibility, With<YoutubeErrorText>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    nav: Res<crate::input::NavInput>,
    theme: Res<UiTheme>,
    active_download: Option<Res<ActiveDownload>>,
) {
    // Close on Escape (only if not downloading)
    if (keyboard.just_pressed(KeyCode::Escape) || nav.back) && active_download.is_none() {
        despawn_youtube_dialog(&mut commands, &dialog_query);
        return;
    }

    // Already downloading - don't process button clicks
    if active_download.is_some() || dialog_state.is_downloading {
        return;
    }

    // Handle Cancel button
    for (interaction, mut bg) in &mut cancel_query {
        match interaction {
            Interaction::Pressed => {
                despawn_youtube_dialog(&mut commands, &dialog_query);
                return;
            }
            Interaction::Hovered => {
                *bg = BackgroundColor(theme.popup_btn_hover);
            }
            Interaction::None => {
                *bg = BackgroundColor(theme.popup_btn);
            }
        }
    }

    // Handle Download button
    for (interaction, mut bg) in &mut download_query {
        match interaction {
            Interaction::Pressed => {
                let url = dialog_state.input_text.trim().to_string();

                // Validate URL
                if !is_valid_youtube_url(&url) {
                    dialog_state.error_message = Some("Please enter a valid YouTube URL".into());
                    if let Ok(mut vis) = error_text_query.single_mut() {
                        *vis = Visibility::Inherited;
                    }
                    continue;
                }

                // Check if folder is selected
                let Some(ref dest_dir) = config.last_folder else {
                    dialog_state.error_message = Some("Please select a music folder first".into());
                    if let Ok(mut vis) = error_text_query.single_mut() {
                        *vis = Visibility::Inherited;
                    }
                    continue;
                };

                // Get downloader and check availability
                let downloader = get_downloader();
                if !downloader.is_available() {
                    dialog_state.error_message = Some(
                        "Video downloader is not installed. Please restart the app to complete setup.".into()
                    );
                    if let Ok(mut vis) = error_text_query.single_mut() {
                        *vis = Visibility::Inherited;
                    }
                    continue;
                }

                // Start download
                dialog_state.is_downloading = true;
                dialog_state.error_message = None;

                let progress = Arc::new(Mutex::new(DownloadStatus::Starting));

                let dest_dir = dest_dir.clone();
                let url_clone = url.clone();
                let progress_clone = Arc::clone(&progress);

                // Spawn download in background thread
                let handle = downloader.download_async(url_clone, dest_dir.clone(), progress_clone);

                // Store both progress and handle
                commands.insert_resource(ActiveDownload {
                    url,
                    dest_dir,
                    progress,
                    handle: Some(handle),
                });
            }
            Interaction::Hovered => {
                *bg = BackgroundColor(theme.accent_hover);
            }
            Interaction::None => {
                *bg = BackgroundColor(theme.accent);
            }
        }
    }
}

pub fn poll_download_progress(
    mut commands: Commands,
    active_download: Option<Res<ActiveDownload>>,
    mut library: ResMut<SongLibrary>,
    mut queue: ResMut<AnalysisQueue>,
    cache: Res<CacheDir>,
    dialog_query: Query<Entity, With<YoutubeDialogRoot>>,
    mut error_text_query: Query<(&mut Text, &mut Visibility), With<YoutubeErrorText>>,
    mut dialog_state: ResMut<YoutubeDialogState>,
) {
    let Some(active) = active_download else {
        return;
    };

    // Safely read the status (handles mutex poisoning)
    let status = lock_mutex_safe(&active.progress).clone();

    match status {
        DownloadStatus::Completed { video_path } => {
            info!("YouTube download completed: {}", video_path.display());

            // Add song to library
            match build_song_from_download(&video_path, &cache) {
                Ok(mut song) => {
                    song.analysis_status = AnalysisStatus::Queued;
                    let idx = library.songs.len();
                    library.songs.push(song.clone());
                    queue.enqueue(idx);
                    info!(
                        "Added {} to library and analysis queue",
                        song.display_title()
                    );
                }
                Err(e) => {
                    error!("Failed to build song from download: {}", e);
                }
            }

            // Cleanup - the thread will naturally complete
            // Status is communicated via Arc<Mutex> so we don't need to join
            commands.remove_resource::<ActiveDownload>();
            despawn_youtube_dialog(&mut commands, &dialog_query);
        }
        DownloadStatus::Failed(ref msg) => {
            error!("YouTube download failed: {}", msg);

            // Show error in dialog
            if let Ok((mut text, mut vis)) = error_text_query.single_mut() {
                **text = msg.clone();
                *vis = Visibility::Inherited;
            }

            commands.remove_resource::<ActiveDownload>();

            // Reset dialog state
            dialog_state.is_downloading = false;
            dialog_state.error_message = Some(msg.clone());
        }
        DownloadStatus::Downloading {
            percent,
            message: _,
        } => {
            debug!("Download progress: {}%", percent);
        }
        DownloadStatus::Starting => {
            debug!("Download starting...");
        }
    }
}
