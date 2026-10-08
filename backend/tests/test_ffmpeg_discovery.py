"""Tests for the ffmpeg discovery that browser-recording decoding depends on.

librosa hands WebM/Opus to ffmpeg via audioread, and a server launched by launchd
or by the packaged app does not inherit Homebrew's bin on PATH. Without this the
user is told their recording "may be empty or corrupt" — the failure looks like
bad audio, not a missing binary.
"""

import os

from backend.utils import audio


def test_known_directory_is_added_to_path(monkeypatch, tmp_path):
    """ffmpeg installed in a known bin dir becomes reachable."""
    binary = tmp_path / "ffmpeg"
    binary.write_text("#!/bin/sh\n")
    binary.chmod(0o755)

    monkeypatch.setattr(audio, "_FFMPEG_DIRS", (str(tmp_path),))
    monkeypatch.setenv("PATH", "/usr/bin:/bin")
    assert audio.shutil.which("ffmpeg") is None, "precondition: no ffmpeg on this PATH"

    audio._ensure_ffmpeg_available()

    assert audio.shutil.which("ffmpeg") == str(binary)
    assert os.environ["PATH"] == f"{tmp_path}{os.pathsep}/usr/bin:/bin"


def test_path_is_left_alone_when_ffmpeg_is_already_reachable(monkeypatch):
    monkeypatch.setattr(audio.shutil, "which", lambda name: "/usr/local/bin/ffmpeg")
    monkeypatch.setenv("PATH", "/usr/bin:/bin")

    audio._ensure_ffmpeg_available()

    assert os.environ["PATH"] == "/usr/bin:/bin"


def test_no_ffmpeg_anywhere_is_not_an_error(monkeypatch, tmp_path):
    """A machine without ffmpeg still runs — only browser recordings degrade."""
    monkeypatch.setattr(audio, "_FFMPEG_DIRS", (str(tmp_path),))
    monkeypatch.setattr(audio.shutil, "which", lambda name: None)
    monkeypatch.setenv("PATH", "/usr/bin:/bin")

    audio._ensure_ffmpeg_available()

    assert os.environ["PATH"] == "/usr/bin:/bin"
