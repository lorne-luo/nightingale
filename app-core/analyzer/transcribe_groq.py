"""Groq Whisper API transcription module."""

import os
import time
import logging
from whisper_compat import progress

try:
    from groq import Groq
except ImportError:
    Groq = None  # Will raise error at runtime if used

logger = logging.getLogger(__name__)


def _get_attr(obj, key, default=None):
    """Get attribute from object (supports both attr and dict access)."""
    if hasattr(obj, key):
        return getattr(obj, key)
    if isinstance(obj, dict):
        return obj.get(key, default)
    return default


def transcribe_with_groq(vocals_path: str) -> dict:
    """
    Transcribe vocals using Groq Whisper API.

    Args:
        vocals_path: Path to the separated vocals audio file.

    Returns:
        Transcript dict with language, segments, and source.
        Each segment has text, start, end, and empty words array.
    """
    if Groq is None:
        raise RuntimeError("groq package not installed. Run: pip install groq")

    api_key = os.environ.get("GROQ_API_KEY")
    if not api_key:
        raise RuntimeError("GROQ_API_KEY not set")

    progress(60, "Calling Groq Whisper API...")

    client = Groq(api_key=api_key)
    audio_size_mb = os.path.getsize(vocals_path) / (1024 * 1024)

    print(f"[nightingale:LOG] [Groq] Starting transcription: file={vocals_path}, size={audio_size_mb:.2f}MB, model=whisper-large-v3-turbo", flush=True)
    logger.info(f"[Groq] Starting transcription: file={vocals_path}, size={audio_size_mb:.2f}MB, model=whisper-large-v3-turbo")
    start_time = time.time()

    try:
        with open(vocals_path, "rb") as audio_file:
            response = client.audio.transcriptions.create(
                file=audio_file,
                model="whisper-large-v3-turbo",
                response_format="verbose_json",
            )
        elapsed = time.time() - start_time
        print(f"[nightingale:LOG] [Groq] Transcription completed: duration={elapsed:.2f}s", flush=True)
        logger.info(f"[Groq] Transcription completed: duration={elapsed:.2f}s")
    except Exception as e:
        elapsed = time.time() - start_time
        print(f"[nightingale:LOG] [Groq] Transcription failed: duration={elapsed:.2f}s, error={e}", flush=True)
        logger.error(f"[Groq] Transcription failed: duration={elapsed:.2f}s, error={e}")
        raise RuntimeError(f"Groq API transcription failed: {e}") from e

    progress(85, "Processing Groq response...")

    # Extract language from response
    language = _get_attr(response, "language", "unknown")

    # Extract segments from response
    segments = []
    raw_segments = _get_attr(response, "segments", [])

    for seg in raw_segments:
        segments.append({
            "text": _get_attr(seg, "text", ""),
            "start": _get_attr(seg, "start", 0.0),
            "end": _get_attr(seg, "end", 0.0),
            "words": [],  # Groq doesn't provide word-level timestamps
        })

    progress(90, f"Groq transcription complete: {len(segments)} segments, language={language}")

    return {
        "language": language,
        "segments": segments,
        "source": "generated",
    }
