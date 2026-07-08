"""Groq Whisper transcription using separated vocal stems."""

import os

from whisper_compat import progress


def transcribe_with_groq(vocals_path: str) -> dict:
    """Transcribe vocals with Groq Whisper and return segment-level transcript."""
    api_key = os.environ.get("GROQ_API_KEY")
    if not api_key:
        raise RuntimeError("GROQ_API_KEY is not set")

    print(f"[nightingale:LOG] Using Groq Whisper for transcription: {vocals_path}", flush=True)
    progress(55, "Uploading vocals to Groq Whisper...")

    from groq import Groq

    client = Groq(api_key=api_key)
    with open(vocals_path, "rb") as audio_file:
        response = client.audio.transcriptions.create(
            file=(os.path.basename(vocals_path), audio_file.read()),
            model="whisper-large-v3-turbo",
            response_format="verbose_json",
        )

    payload = response.model_dump() if hasattr(response, "model_dump") else dict(response)
    language = payload.get("language") or "unknown"
    raw_segments = payload.get("segments") or []

    segments = []
    for idx, seg in enumerate(raw_segments):
        text = (seg.get("text") or "").strip()
        start = round(float(seg.get("start", 0.0)), 3)
        end = round(float(seg.get("end", start)), 3)
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

    progress(90, f"Groq transcription complete: {len(segments)} segments, lang={language}")
    return {
        "language": language,
        "segments": segments,
        "source": "generated",
    }
