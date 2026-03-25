use std::path::Path;

// Include Python scripts from app-core's analyzer directory
const ANALYZE_PY: &str = include_str!("../../app-core/analyzer/analyze.py");
const SERVER_PY: &str = include_str!("../../app-core/analyzer/server.py");
const PIPELINE_PY: &str = include_str!("../../app-core/analyzer/pipeline.py");
const STEMS_PY: &str = include_str!("../../app-core/analyzer/stems.py");
const TRANSCRIBE_PY: &str = include_str!("../../app-core/analyzer/transcribe.py");
const TRANSCRIBE_GROQ_PY: &str = include_str!("../../app-core/analyzer/transcribe_groq.py");
const ALIGN_PY: &str = include_str!("../../app-core/analyzer/align.py");
const AUDIO_PY: &str = include_str!("../../app-core/analyzer/audio.py");
const HALLUCINATION_PY: &str = include_str!("../../app-core/analyzer/hallucination.py");
const LANGUAGE_PY: &str = include_str!("../../app-core/analyzer/language.py");
const WHISPER_COMPAT_PY: &str = include_str!("../../app-core/analyzer/whisper_compat.py");
const REQUIREMENTS_TXT: &str = include_str!("../../app-core/analyzer/requirements.txt");

const FILES: &[(&str, &str)] = &[
    ("analyze.py", ANALYZE_PY),
    ("server.py", SERVER_PY),
    ("pipeline.py", PIPELINE_PY),
    ("stems.py", STEMS_PY),
    ("transcribe.py", TRANSCRIBE_PY),
    ("transcribe_groq.py", TRANSCRIBE_GROQ_PY),
    ("align.py", ALIGN_PY),
    ("audio.py", AUDIO_PY),
    ("hallucination.py", HALLUCINATION_PY),
    ("language.py", LANGUAGE_PY),
    ("whisper_compat.py", WHISPER_COMPAT_PY),
    ("requirements.txt", REQUIREMENTS_TXT),
];

pub fn write_scripts(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    for (name, content) in FILES {
        std::fs::write(dir.join(name), content)?;
    }
    Ok(())
}