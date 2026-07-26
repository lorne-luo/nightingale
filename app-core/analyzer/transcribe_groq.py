"""Groq Whisper API transcription module."""

import logging
import os
import time

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
    """Transcribe vocals with Groq Whisper and return segment-level transcript."""
    if Groq is None:
        raise RuntimeError("groq package not installed. Run: pip install groq")

    api_key = os.environ.get("GROQ_API_KEY")
    if not api_key:
        raise RuntimeError("GROQ_API_KEY not set")

    progress(60, "Calling Groq Whisper API...")

    client = Groq(api_key=api_key)
    audio_size_mb = os.path.getsize(vocals_path) / (1024 * 1024)

    message = (
        f"[Groq] Starting transcription: file={vocals_path}, "
        f"size={audio_size_mb:.2f}MB, model=whisper-large-v3-turbo"
    )
    print(f"[nightingale:LOG] {message}", flush=True)
    logger.info(message)
    start_time = time.time()

    try:
        with open(vocals_path, "rb") as audio_file:
            response = client.audio.transcriptions.create(
                file=(os.path.basename(vocals_path), audio_file.read()),
                model="whisper-large-v3-turbo",
                response_format="verbose_json",
            )
        elapsed = time.time() - start_time
        print(
            f"[nightingale:LOG] [Groq] Transcription completed: duration={elapsed:.2f}s",
            flush=True,
        )
        logger.info(f"[Groq] Transcription completed: duration={elapsed:.2f}s")
    except Exception as e:
        elapsed = time.time() - start_time
        print(
            f"[nightingale:LOG] [Groq] Transcription failed: "
            f"duration={elapsed:.2f}s, error={e}",
            flush=True,
        )
        logger.error(f"[Groq] Transcription failed: duration={elapsed:.2f}s, error={e}")
        raise RuntimeError(f"Groq API transcription failed: {e}") from e

    progress(85, "Processing Groq response...")

    language = _get_attr(response, "language", "unknown") or "unknown"
    segments = []
    for idx, seg in enumerate(_get_attr(response, "segments", []) or []):
        text = str(_get_attr(seg, "text", "") or "").strip()
        start = round(float(_get_attr(seg, "start", 0.0)), 3)
        end = round(float(_get_attr(seg, "end", start)), 3)
        if not text:
            continue
        if end < start:
            end = start
        segments.append({
            "text": text,
            "start": start,
            "end": end,
            "words": [],
        })
        print(
            f"[nightingale:LOG] Groq seg {idx}: [{start:.1f}-{end:.1f}] {text[:80]}",
            flush=True,
        )

    progress(90, f"Groq transcription complete: {len(segments)} segments, language={language}")
    return {
        "language": language,
        "segments": segments,
        "source": "generated",
    }
