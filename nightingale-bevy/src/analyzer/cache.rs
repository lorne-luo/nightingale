use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct CacheDir {
    pub path: PathBuf,
}

impl CacheDir {
    pub fn new() -> Self {
        let path = nightingale_dir().join("cache");
        std::fs::create_dir_all(&path).expect("could not create cache directory");
        Self { path }
    }

    pub fn transcript_path(&self, hash: &str) -> PathBuf {
        self.path.join(format!("{hash}_transcript.json"))
    }

    pub fn instrumental_path(&self, hash: &str) -> PathBuf {
        self.path.join(format!("{hash}_instrumental.mp3"))
    }

    pub fn vocals_path(&self, hash: &str) -> PathBuf {
        self.path.join(format!("{hash}_vocals.mp3"))
    }

    pub fn legacy_instrumental_path(&self, hash: &str) -> PathBuf {
        self.path.join(format!("{hash}_instrumental.ogg"))
    }

    pub fn legacy_vocals_path(&self, hash: &str) -> PathBuf {
        self.path.join(format!("{hash}_vocals.ogg"))
    }

    fn stems_exist(&self, hash: &str) -> bool {
        (self.instrumental_path(hash).is_file() && self.vocals_path(hash).is_file())
            || (self.legacy_instrumental_path(hash).is_file()
                && self.legacy_vocals_path(hash).is_file())
    }

    pub fn lyrics_path(&self, hash: &str) -> PathBuf {
        self.path.join(format!("{hash}_lyrics.json"))
    }

    pub fn cover_path(&self, hash: &str) -> PathBuf {
        self.path.join(format!("{hash}_cover.jpg"))
    }

    pub fn transcript_exists(&self, hash: &str) -> bool {
        self.transcript_path(hash).is_file() && self.stems_exist(hash)
    }
}

pub fn nightingale_dir() -> PathBuf {
    dirs::home_dir()
        .expect("could not find home directory")
        .join(".nightingale")
}

pub fn config_path() -> PathBuf {
    nightingale_dir().join("config.json")
}

pub fn profiles_path() -> PathBuf {
    nightingale_dir().join("profiles.json")
}

pub fn models_dir() -> PathBuf {
    nightingale_dir().join("models")
}

pub fn videos_dir() -> PathBuf {
    nightingale_dir().join("videos")
}

pub fn dir_size(path: &Path) -> u64 {
    if !path.is_dir() {
        return 0;
    }

    WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}