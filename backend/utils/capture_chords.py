"""Platform defaults for capture hotkey chords."""

from __future__ import annotations

import sys


# Chords are W3C KeyboardEvent.code names (see tauri/src-tauri/src/key_codes.rs).
MAC_PUSH_TO_TALK = ["MetaRight", "AltGr"]
MAC_TOGGLE_TO_TALK = ["MetaRight", "AltGr", "Space"]
# Agent voice: stop talking now, and mute/unmute agent speech entirely.
MAC_VOICE_STOP = ["MetaRight", "AltGr", "KeyX"]
MAC_VOICE_TOGGLE = ["MetaRight", "AltGr", "KeyM"]
# Hold to talk to your agent instead of the focused field.
MAC_AGENT = ["MetaRight", "AltGr", "KeyA"]
NON_MAC_AGENT = ["ControlRight", "ShiftRight", "KeyA"]
NON_MAC_VOICE_STOP = ["ControlRight", "ShiftRight", "KeyX"]
NON_MAC_VOICE_TOGGLE = ["ControlRight", "ShiftRight", "KeyM"]

NON_MAC_PUSH_TO_TALK = ["ControlRight", "ShiftRight"]
NON_MAC_TOGGLE_TO_TALK = ["ControlRight", "ShiftRight", "Space"]


def default_push_to_talk_chord() -> list[str]:
    if sys.platform == "darwin":
        return MAC_PUSH_TO_TALK.copy()
    return NON_MAC_PUSH_TO_TALK.copy()


def default_toggle_to_talk_chord() -> list[str]:
    if sys.platform == "darwin":
        return MAC_TOGGLE_TO_TALK.copy()
    return NON_MAC_TOGGLE_TO_TALK.copy()


def default_voice_stop_chord() -> list[str]:
    """Chord that silences agent speech immediately (narration, playback, pill)."""
    if sys.platform == "darwin":
        return MAC_VOICE_STOP.copy()
    return NON_MAC_VOICE_STOP.copy()


def default_voice_toggle_chord() -> list[str]:
    """Chord that mutes or unmutes agent speech until pressed again."""
    if sys.platform == "darwin":
        return MAC_VOICE_TOGGLE.copy()
    return NON_MAC_VOICE_TOGGLE.copy()


def default_agent_chord() -> list[str]:
    """Chord held to dictate into an agent session rather than the focused field."""
    if sys.platform == "darwin":
        return MAC_AGENT.copy()
    return NON_MAC_AGENT.copy()
