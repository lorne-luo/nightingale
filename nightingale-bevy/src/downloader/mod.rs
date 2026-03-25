pub mod dialog;
pub mod yt_dlp;

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;

use crate::scanner::metadata::Song;

// ─────────────────────────────────────────────────────────────────────────────
// Error Types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum DownloadError {
    NotInstalled,
    InvalidUrl(String),
    DownloadFailed(String),
    NetworkError(String),
}

impl std::fmt::Display for DownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotInstalled => write!(f, "Video downloader is not installed"),
            Self::InvalidUrl(url) => write!(f, "Invalid URL: {}", url),
            Self::DownloadFailed(msg) => write!(f, "Download failed: {}", msg),
            Self::NetworkError(msg) => write!(f, "Network error: {}", msg),
        }
    }
}

pub type DownloadResult = Result<PathBuf, DownloadError>;

// ─────────────────────────────────────────────────────────────────────────────
// VideoDownloader Trait (extensible design)
// ─────────────────────────────────────────────────────────────────────────────

pub trait VideoDownloader: Send + Sync {
    /// Returns the name of the downloader for logging/UI purposes
    fn name(&self) -> &str;

    /// Checks if the downloader is available on the system
    fn is_available(&self) -> bool;

    /// Downloads a video synchronously
    fn download(&self, url: &str, dest_dir: &std::path::Path) -> DownloadResult;

    /// Downloads a video asynchronously, returning a join handle
    fn download_async(
        &self,
        url: String,
        dest_dir: PathBuf,
        progress: Arc<Mutex<DownloadStatus>>,
    ) -> std::thread::JoinHandle<DownloadResult>;
}

/// Factory function to get the current downloader
/// This can be modified in the future to support configuration-based selection
pub fn get_downloader() -> Box<dyn VideoDownloader> {
    Box::new(yt_dlp::YtDlpDownloader::new())
}

// ─────────────────────────────────────────────────────────────────────────────
// Download Status & Progress
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub url: String,
    pub status: DownloadStatus,
    pub dest_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub enum DownloadStatus {
    Starting,
    Downloading { percent: u32, message: String },
    Completed { video_path: PathBuf },
    Failed(String),
}

// ─────────────────────────────────────────────────────────────────────────────
// Resources
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Resource)]
pub struct ActiveDownload {
    pub url: String,
    pub dest_dir: PathBuf,
    pub progress: Arc<Mutex<DownloadStatus>>,
    /// Store the join handle to detect thread panics
    pub handle: Option<std::thread::JoinHandle<DownloadResult>>,
}

#[derive(Component)]
pub struct YoutubeDialogRoot;

#[derive(Component)]
pub struct YoutubeUrlInputText;

#[derive(Component)]
pub struct YoutubeCancelButton;

#[derive(Component)]
pub struct YoutubeDownloadButton;

#[derive(Component)]
pub struct YoutubeErrorText;

#[derive(Resource, Default)]
pub struct YoutubeDialogState {
    pub input_text: String,
    pub error_message: Option<String>,
    pub is_downloading: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// URL Validation (proper parsing)
// ─────────────────────────────────────────────────────────────────────────────

/// Validates a YouTube URL using proper parsing
pub fn is_valid_youtube_url(url: &str) -> bool {
    let url = url.trim();

    // Must have a valid scheme
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return false;
    }

    // Check for valid YouTube domains
    let lower = url.to_lowercase();
    let is_youtube = lower.contains("youtube.com/watch")
        || lower.contains("www.youtube.com/watch")
        || lower.contains("m.youtube.com/watch")
        || lower.contains("youtu.be/");

    if !is_youtube {
        return false;
    }

    // Must have a video ID
    extract_video_id(url).is_some()
}

/// Extracts the YouTube video ID from a URL
pub fn extract_video_id(url: &str) -> Option<String> {
    let url = url.trim();

    // Handle youtu.be short URLs
    if url.contains("youtu.be/") {
        let start = url.find("youtu.be/")? + 9;
        let rest = &url[start..];
        // Find end of video ID (before query params or other path segments)
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

    // Handle youtube.com URLs with v= parameter
    if let Some(pos) = url.find("v=") {
        let start = pos + 2;
        let rest = &url[start..];
        // Find end of video ID (before next query param)
        let end = rest.find('&').unwrap_or(rest.len());
        let id = &rest[..end];
        // YouTube video IDs are typically 11 characters
        if id.len() >= 11 && id.len() <= 12 {
            return Some(id.to_string());
        }
    }

    None
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper to safely read mutex (handles poisoning)
// ─────────────────────────────────────────────────────────────────────────────

/// Safely locks a mutex, recovering from poisoning if necessary
pub fn lock_mutex_safe<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poison) => poison.into_inner(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Song Building
// ─────────────────────────────────────────────────────────────────────────────

pub fn build_song_from_download(
    video_path: &std::path::Path,
    _cache: &crate::analyzer::cache::CacheDir,
) -> Result<Song, crate::error::NightingaleError> {
    let file_hash = crate::scanner::compute_file_hash(video_path)?;
    let analysis_status = crate::scanner::metadata::AnalysisStatus::NotAnalyzed;

    Ok(Song::from_path(
        video_path,
        file_hash,
        analysis_status,
        None,
        true,
    ))
}

// ─────────────────────────────────────────────────────────────────────────────
// Dialog UI
// ─────────────────────────────────────────────────────────────────────────────

pub fn spawn_youtube_dialog(commands: &mut Commands, theme: &crate::ui::UiTheme) {
    commands.insert_resource(YoutubeDialogState::default());

    commands
        .spawn((
            YoutubeDialogRoot,
            GlobalZIndex(100),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(theme.overlay_dim),
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    Node {
                        width: Val::Px(400.0),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(Val::Px(24.0)),
                        row_gap: Val::Px(16.0),
                        border_radius: BorderRadius::all(Val::Px(8.0)),
                        ..default()
                    },
                    BackgroundColor(theme.surface),
                ))
                .with_children(|card| {
                    // Title
                    card.spawn((
                        Text::new("Download from YouTube"),
                        TextFont {
                            font_size: 20.0,
                            ..default()
                        },
                        TextColor(theme.text_primary),
                    ));

                    // Input field container
                    card.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(40.0),
                            padding: UiRect::horizontal(Val::Px(12.0)),
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(1.0)),
                            border_radius: BorderRadius::all(Val::Px(4.0)),
                            ..default()
                        },
                        BackgroundColor(theme.card_bg),
                        BorderColor::all(theme.text_dim.with_alpha(0.3)),
                    ))
                    .with_children(|input| {
                        // Input text (will be updated by input handler)
                        input.spawn((
                            YoutubeUrlInputText,
                            Text::new("Paste YouTube link..."),
                            TextFont {
                                font_size: 14.0,
                                ..default()
                            },
                            TextColor(theme.text_dim),
                        ));
                    });

                    // Error text
                    card.spawn((
                        YoutubeErrorText,
                        Text::new(""),
                        TextFont {
                            font_size: 12.0,
                            ..default()
                        },
                        TextColor(theme.badge_failed),
                        Visibility::Hidden,
                    ));

                    // Buttons row
                    card.spawn((Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::FlexEnd,
                        column_gap: Val::Px(8.0),
                        ..default()
                    },))
                        .with_children(|buttons| {
                            // Cancel button
                            buttons
                                .spawn((
                                    YoutubeCancelButton,
                                    Button,
                                    Node {
                                        padding: UiRect::new(
                                            Val::Px(16.0),
                                            Val::Px(16.0),
                                            Val::Px(8.0),
                                            Val::Px(8.0),
                                        ),
                                        border_radius: BorderRadius::all(Val::Px(4.0)),
                                        ..default()
                                    },
                                    BackgroundColor(theme.popup_btn),
                                ))
                                .with_children(|btn| {
                                    btn.spawn((
                                        Text::new("Cancel"),
                                        TextFont {
                                            font_size: 14.0,
                                            ..default()
                                        },
                                        TextColor(theme.text_secondary),
                                    ));
                                });

                            // Download button
                            buttons
                                .spawn((
                                    YoutubeDownloadButton,
                                    Button,
                                    Node {
                                        padding: UiRect::new(
                                            Val::Px(16.0),
                                            Val::Px(16.0),
                                            Val::Px(8.0),
                                            Val::Px(8.0),
                                        ),
                                        border_radius: BorderRadius::all(Val::Px(4.0)),
                                        ..default()
                                    },
                                    BackgroundColor(theme.accent),
                                ))
                                .with_children(|btn| {
                                    btn.spawn((
                                        Text::new("Download"),
                                        TextFont {
                                            font_size: 14.0,
                                            ..default()
                                        },
                                        TextColor(theme.text_primary),
                                    ));
                                });
                        });
                });
        });
}

pub fn despawn_youtube_dialog(
    commands: &mut Commands,
    query: &Query<Entity, With<YoutubeDialogRoot>>,
) {
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<YoutubeDialogState>();
}

/// Handles text input for the YouTube dialog
pub fn handle_youtube_text_input(
    mut key_events: MessageReader<KeyboardInput>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut dialog_state: ResMut<YoutubeDialogState>,
    mut input_text_query: Query<(&mut Text, &mut TextColor), With<YoutubeUrlInputText>>,
    theme: Res<crate::ui::UiTheme>,
    dialog_query: Query<(), With<YoutubeDialogRoot>>,
    active_download: Option<Res<ActiveDownload>>,
) {
    // Only process if dialog is open and not downloading
    if dialog_query.is_empty() || active_download.is_some() || dialog_state.is_downloading {
        return;
    }

    let mut changed = false;

    for ev in key_events.read() {
        if !ev.state.is_pressed() {
            continue;
        }

        // Handle backspace
        if ev.key_code == KeyCode::Backspace {
            if !dialog_state.input_text.is_empty() {
                dialog_state.input_text.pop();
                changed = true;
            }
            continue;
        }

        // Handle paste (Cmd+V on Mac, Ctrl+V on other platforms)
        #[cfg(target_os = "macos")]
        let is_paste = ev.key_code == KeyCode::KeyV
            && (keyboard.pressed(KeyCode::SuperLeft) || keyboard.pressed(KeyCode::SuperRight));
        #[cfg(not(target_os = "macos"))]
        let is_paste = ev.key_code == KeyCode::KeyV
            && (keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight));

        if is_paste {
            // Get clipboard content
            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                if let Ok(text) = clipboard.get_text() {
                    for c in text.chars() {
                        if !c.is_control() {
                            dialog_state.input_text.push(c);
                        }
                    }
                    changed = true;
                }
            }
            continue;
        }

        // Handle regular character input
        if let Some(ref text) = ev.text {
            for c in text.chars() {
                if !c.is_control() {
                    dialog_state.input_text.push(c);
                    changed = true;
                }
            }
        }
    }

    if !changed {
        return;
    }

    // Update the displayed text
    if let Ok((mut text, mut color)) = input_text_query.single_mut() {
        if dialog_state.input_text.is_empty() {
            **text = "Paste YouTube link...".into();
            *color = TextColor(theme.text_dim);
        } else {
            **text = dialog_state.input_text.clone();
            *color = TextColor(theme.text_primary);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Plugin
// ─────────────────────────────────────────────────────────────────────────────

pub struct DownloaderPlugin;

impl Plugin for DownloaderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<YoutubeDialogState>().add_systems(
            Update,
            (
                handle_youtube_text_input,
                dialog::handle_youtube_dialog,
                dialog::poll_download_progress,
            )
                .run_if(in_state(crate::states::AppState::Menu)),
        );
    }
}
