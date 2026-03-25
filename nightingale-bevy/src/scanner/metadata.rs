use std::path::PathBuf;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TranscriptSource {
    Lyrics,
    Generated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnalysisStatus {
    NotAnalyzed,
    Analyzing,
    Ready(TranscriptSource),
    Failed(String),
}

impl Default for AnalysisStatus {
    fn default() -> Self {
        Self::NotAnalyzed
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Component)]
pub struct Song {
    pub path: PathBuf,
    pub file_hash: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_secs: f64,
    pub album_art_path: Option<PathBuf>,
    pub analysis_status: AnalysisStatus,
    pub language: Option<String>,
    pub is_video: bool,
}

impl Song {
    pub fn from_path(
        path: &std::path::Path,
        file_hash: String,
        analysis_status: AnalysisStatus,
        language: Option<String>,
        is_video: bool,
    ) -> Self {
        // Simplified implementation - real implementation would read metadata
        let title = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown")
            .to_string();

        Self {
            path: path.to_path_buf(),
            file_hash,
            title,
            artist: "Unknown Artist".to_string(),
            album: "Unknown Album".to_string(),
            duration_secs: 0.0,
            album_art_path: None,
            analysis_status,
            language,
            is_video,
        }
    }

    pub fn display_title(&self) -> &str {
        &self.title
    }

    pub fn display_artist(&self) -> &str {
        &self.artist
    }
}

#[derive(Resource, Default)]
pub struct SongLibrary {
    pub songs: Vec<Song>,
}