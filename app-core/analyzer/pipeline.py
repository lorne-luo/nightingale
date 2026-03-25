"""Shared analysis pipeline used by both server.py and analyze.py."""

import json
import os
import subprocess
import tempfile

from whisper_compat import progress
from stems import separate_stems, separate_stems_uvr
from transcribe import transcribe_vocals
from transcribe_groq import transcribe_with_groq
from align import align_lyrics


def ffmpeg_bin():
    return os.environ.get("FFMPEG_PATH", "ffmpeg")


def convert_to_mp3(src, dest_mp3):
    subprocess.run(
        [ffmpeg_bin(), "-y", "-i", src, "-c:a", "libmp3lame", "-q:a", "2", "-v", "error", dest_mp3],
        check=True,
    )
    if os.path.isfile(dest_mp3):
        os.remove(src)


def separate_and_cache(audio_path, output_dir, file_hash, separator, device, free_gpu_fn=None):
    """Run stem separation or reuse cached stems. Returns the vocals path."""
    final_vocals = os.path.join(output_dir, f"{file_hash}_vocals.mp3")
    final_instrumental = os.path.join(output_dir, f"{file_hash}_instrumental.mp3")

    if os.path.isfile(final_vocals) and os.path.isfile(final_instrumental):
        progress(50, "Stems already cached, skipping separation")
        return final_vocals

    for ext in (".ogg", ".wav"):
        legacy_v = os.path.join(output_dir, f"{file_hash}_vocals{ext}")
        legacy_i = os.path.join(output_dir, f"{file_hash}_instrumental{ext}")
        if os.path.isfile(legacy_v) and os.path.isfile(legacy_i):
            progress(50, f"Converting legacy {ext} stems to MP3...")
            convert_to_mp3(legacy_v, final_vocals)
            convert_to_mp3(legacy_i, final_instrumental)
            return final_vocals

    with tempfile.TemporaryDirectory(prefix="nightingale_") as work_dir:
        if separator == "karaoke":
            torch_home = os.environ.get("TORCH_HOME", "")
            models_base = os.path.dirname(torch_home) if torch_home else output_dir
            uvr_models_dir = os.path.join(models_base, "audio_separator")
            os.makedirs(uvr_models_dir, exist_ok=True)
            vp, ip = separate_stems_uvr(audio_path, work_dir, uvr_models_dir)
        else:
            vp, ip = separate_stems(audio_path, work_dir, device)
        progress(51, "Saving stems to cache...")
        convert_to_mp3(vp, final_vocals)
        convert_to_mp3(ip, final_instrumental)

    if free_gpu_fn:
        free_gpu_fn()

    return final_vocals


def transcribe_or_align(
    vocals_path, audio_path, device, *,
    model_name, beam_size=5, batch_size=16,
    lyrics_path=None, language_override=None,
    whisper_model=None, pre_align_cleanup=None,
):
    """Choose between lyrics alignment and full transcription."""
    if lyrics_path and os.path.isfile(lyrics_path):
        print(f"[nightingale:LOG] Using pre-fetched lyrics: {lyrics_path}", flush=True)
        if callable(whisper_model):
            whisper_model = whisper_model()
        return align_lyrics(
            lyrics_path, vocals_path, device,
            model_name=model_name,
            language_override=language_override,
            whisper_model=whisper_model,
            pre_align_cleanup=pre_align_cleanup,
        )

    if os.environ.get("GROQ_API_KEY"):
        return transcribe_with_groq(vocals_path)

    if callable(whisper_model):
        whisper_model = whisper_model()

    return transcribe_vocals(
        vocals_path, audio_path, device,
        model_name=model_name,
        beam_size=beam_size,
        batch_size=batch_size,
        language_override=language_override,
        whisper_model=whisper_model,
        pre_align_cleanup=pre_align_cleanup,
    )


def run_pipeline(
    audio_path, output_dir, file_hash, device, *,
    model_name="large-v3", beam_size=5, batch_size=16,
    separator="karaoke", lyrics_path=None, language_override=None,
    whisper_model=None, pre_align_cleanup=None, free_gpu_fn=None,
):
    """Full analysis pipeline: stem separation -> transcription -> save."""
    os.makedirs(output_dir, exist_ok=True)

    transcript_path = os.path.join(output_dir, f"{file_hash}_transcript.json")
    if os.path.isfile(transcript_path):
        progress(100, "Already analyzed, skipping")
        return

    progress(2, f"Using device: {device}")

    vocals_path = separate_and_cache(
        audio_path, output_dir, file_hash, separator, device,
        free_gpu_fn=free_gpu_fn,
    )

    transcript = transcribe_or_align(
        vocals_path, audio_path, device,
        model_name=model_name,
        beam_size=beam_size,
        batch_size=batch_size,
        lyrics_path=lyrics_path,
        language_override=language_override,
        whisper_model=whisper_model,
        pre_align_cleanup=pre_align_cleanup,
    )

    progress(95, "Writing transcript...")
    with open(transcript_path, "w", encoding="utf-8") as f:
        json.dump(transcript, f, ensure_ascii=False, indent=2)
