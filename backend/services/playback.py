"""Local playback for a headless backend.

Agent speech normally plays through the floating pill in the desktop app. A
backend with no pill attached — the app closed, or a remote/headless server —
would otherwise synthesize audio that nobody hears. When the ``headless_playback``
setting is on and no pill client is subscribed to ``/events/speak``, the backend
plays the file itself through the OS.

Off by default: the pill is deliberately the only surface for agent speech ("no
silent background TTS"), so this is opt-in for setups without one.
"""

from __future__ import annotations

import asyncio
import logging
import time
import shutil
import sys
from pathlib import Path

logger = logging.getLogger(__name__)

# One player at a time: two speaks finishing together must not talk over each
# other.
_play_lock = asyncio.Lock()
# The in-flight headless player, so the user can silence it from a chord.
_current_player: "asyncio.subprocess.Process | None" = None


def _player_command() -> list[str] | None:
    """The OS command that plays a WAV file, or None when there is none."""
    if sys.platform == "darwin":
        return ["afplay"]
    if sys.platform == "win32":
        # Untested here; PlaySync blocks until the file finishes, which is what
        # the awaited subprocess expects.
        return [
            "powershell",
            "-NoProfile",
            "-Command",
            "(New-Object Media.SoundPlayer $args[0]).PlaySync()",
        ]
    for candidate in ("paplay", "aplay"):
        if shutil.which(candidate):
            return [candidate]
    return None


# Evidence that a client actually took a generation's audio. The pill fetches
# /audio/{id} the moment it plays, so a fetch is a reliable ack. Without one we
# must not assume the user can hear anything, however attached the pill looks —
# a pill that dropped its cycle looks exactly like a pill that is playing.
_audio_served: dict[str, float] = {}
_AUDIO_SERVED_LIMIT = 64


def mark_audio_served(generation_id: str) -> None:
    """Record that a client fetched *generation_id*'s audio (so it will play)."""
    _audio_served[generation_id] = time.monotonic()
    if len(_audio_served) > _AUDIO_SERVED_LIMIT:
        oldest = min(_audio_served, key=_audio_served.__getitem__)
        _audio_served.pop(oldest, None)


def audio_served(generation_id: str) -> bool:
    """Whether a client has taken *generation_id*'s audio to play."""
    return generation_id in _audio_served


def stop_playback() -> bool:
    """
    Terminate the in-flight headless player, if any. Returns whether one was killed.

    The play loop is awaited inside a lock, so terminating the process makes that
    await return normally — no exception path, no stuck lock.
    """
    process = _current_player
    if process is None or process.returncode is not None:
        return False
    try:
        process.terminate()
    except ProcessLookupError:
        return False
    return True


def pill_attached() -> bool:
    """Whether a client (the pill) is subscribed to speak events."""
    from ..mcp_server import events as mcp_events

    return mcp_events.has_subscribers()


async def play_file(path: Path) -> None:
    """Play *path* through the OS, serialized against other playback."""
    command = _player_command()
    if command is None:
        logger.warning("Headless playback is on but no audio player was found on this system")
        return

    global _current_player
    async with _play_lock:
        try:
            process = await asyncio.create_subprocess_exec(
                *command,
                str(path),
                stdout=asyncio.subprocess.DEVNULL,
                stderr=asyncio.subprocess.DEVNULL,
            )
            _current_player = process
            try:
                await process.wait()
            finally:
                _current_player = None
        except FileNotFoundError:
            logger.warning("Headless playback: %s is not installed", command[0])
        except Exception:
            logger.exception("Headless playback failed for %s", path)
