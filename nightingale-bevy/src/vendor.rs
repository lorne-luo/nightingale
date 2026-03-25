use std::{path::PathBuf, process::Command};

use crate::analyzer::cache::{models_dir, nightingale_dir};

pub fn vendor_dir() -> PathBuf {
    nightingale_dir().join("vendor")
}

pub fn clear_vendor_dir() -> Result<(), String> {
    let dir = vendor_dir();
    if dir.is_dir() {
        std::fs::remove_dir_all(&dir).map_err(|e| format!("Failed to clear vendor directory: {e}"))?;
    }
    Ok(())
}

pub fn ffmpeg_path() -> PathBuf {
    let name = if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    vendor_dir().join(name)
}

pub fn python_path() -> PathBuf {
    if cfg!(windows) {
        vendor_dir().join("venv").join("Scripts").join("python.exe")
    } else {
        vendor_dir().join("venv").join("bin").join("python")
    }
}

pub fn venv_bin_dir() -> PathBuf {
    if cfg!(windows) {
        vendor_dir().join("venv").join("Scripts")
    } else {
        vendor_dir().join("venv").join("bin")
    }
}

pub fn analyzer_dir() -> PathBuf {
    vendor_dir().join("analyzer")
}

pub fn silent_command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

fn ready_marker() -> PathBuf {
    vendor_dir().join(".ready")
}

pub fn is_ready() -> bool {
    ready_marker().is_file()
        && ffmpeg_path().is_file()
        && python_path().is_file()
        && analyzer_dir().join("analyze.py").is_file()
}

pub fn reset() {
    let _ = clear_vendor_dir();
}

pub fn clear_videos() {
    let base = nightingale_dir().join("videos");
    if base.is_dir() {
        let _ = std::fs::remove_dir_all(&base);
    }
}

pub fn clear_models() {
    let dir = models_dir();
    if dir.is_dir() {
        let _ = std::fs::remove_dir_all(&dir);
    }
}

pub fn clearable_video_bytes() -> u64 {
    use walkdir::WalkDir;
    use std::path::Path;

    let base = nightingale_dir().join("videos");
    if !base.is_dir() {
        return 0;
    }

    let mut total: u64 = 0;
    for entry in std::fs::read_dir(&base).into_iter().flatten().flatten() {
        let flavor_dir = entry.path();
        if !flavor_dir.is_dir() {
            continue;
        }

        let mut mp4s: Vec<_> = std::fs::read_dir(&flavor_dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "mp4"))
            .collect();
        mp4s.sort();

        for path in mp4s.into_iter().skip(1) {
            total += path.metadata().map(|m| m.len()).unwrap_or(0);
        }
    }
    total
}

pub enum BootstrapProgress {
    Step { name: String, message: String },
    Done,
}

pub fn run_bootstrap(tx: std::sync::mpsc::Sender<BootstrapProgress>) {
    // Placeholder - actual bootstrap logic would be more complex
    let _ = tx.send(BootstrapProgress::Done);
}