"""Groq Whisper API transcription module."""

import os
from whisper_compat import progress


def transcribe_with_groq(vocals_path: str) -> dict:
    """
    Transcribe vocals using Groq Whisper API.

    Args:
        vocals_path: Path to the separated vocals audio file.

    Returns:
        Transcript dict with language, segments, and source.
        Each segment has text, start, end, and empty words array.
    """
    from groq import Groq

    api_key = os.environ.get("GROQ_API_KEY")
    if not api_key:
        raise RuntimeError("GROQ_API_KEY not set")

    progress(60, "Calling Groq Whisper API...")

    client = Groq(api_key=api_key)

    with open(vocals_path, "rb") as audio_file:
        response = client.audio.transcriptions.create(
            file=audio_file,
            model="whisper-large-v3-turbo",
            response_format="verbose_json",
        )

    progress(85, "Processing Groq response...")

    # Extract language from response
    language = getattr(response, "language", "unknown")

    # Extract segments from response
    segments = []
    raw_segments = getattr(response, "segments", [])

    for seg in raw_segments:
        segments.append({
            "text": seg.text if hasattr(seg, "text") else seg.get("text", ""),
            "start": seg.start if hasattr(seg, "start") else seg.get("start", 0.0),
            "end": seg.end if hasattr(seg, "end") else seg.get("end", 0.0),
            "words": [],  # Groq doesn't provide word-level timestamps
        })

    progress(90, f"Groq transcription complete: {len(segments)} segments, language={language}")

    return {
        "language": language,
        "segments": segments,
        "source": "generated",
    }
