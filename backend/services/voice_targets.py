"""Deliver a spoken transcript into a running agent session.

The user talks; Voicebox has text. Until now that text could only be pasted
into whatever field had focus, which means the user must be sitting in the
agent's terminal. This module is the other half: hand the text to the agent's
session directly.

The transport is `herdr` — a terminal workspace manager that recognises the
coding agents running in its panes. It is the right primitive for three
reasons beyond reach: it *validates* that a target really is an agent pane
before accepting input (so a mis-addressed message cannot land in a shell), it
reports lifecycle state (`idle`/`working`/`blocked`/`done`), and it needs no
environment from the caller — verified from a scrubbed environment, which
matters because the server runs under launchd, outside any herdr session.

Interface (authoritative from `herdr api schema`, protocol 22):

    herdr agent list                 -> {"id": ..., "result": {"agents": [...]}}
    herdr agent prompt <T> <TEXT>    -> {"id": ..., "result": {...}}
    failure                          -> {"id": ..., "error": {"code", "message"}}
"""

from __future__ import annotations

import json
import logging
import shutil
import subprocess
from dataclasses import dataclass
from pathlib import Path

logger = logging.getLogger(__name__)

#: Prefix that marks a delivery as a *voice turn*. The agent's instruction file
#: teaches it: a message that starts with this must be answered aloud, briefly.
#: It is applied only on the agent path — never to a pasted transcript, which
#: would pollute whatever field the user is dictating into.
VOICE_TURN_MARKER = "[voice turn]"

VOICE_TURN_INSTRUCTION = f"{VOICE_TURN_MARKER} reply aloud, two sentences max."

#: The server runs under launchd with a minimal PATH, so look in the places
#: herdr actually installs itself before giving up.
_FALLBACK_BINARIES = (
    "/opt/homebrew/bin/herdr",
    "/usr/local/bin/herdr",
    "~/.local/bin/herdr",
)

#: `herdr agent list` should be instant; a hung server must not hang a request.
_LIST_TIMEOUT_SECONDS = 10
_PROMPT_TIMEOUT_SECONDS = 30

#: Statuses herdr reports for a pane occupant.
AGENT_STATES = ("idle", "working", "blocked", "done", "unknown")


class VoiceTargetError(RuntimeError):
    """A send that could not be delivered, with herdr's own reason attached."""

    def __init__(self, message: str, *, code: str | None = None) -> None:
        super().__init__(message)
        self.code = code

    @property
    def blocked(self) -> bool:
        """Whether herdr refused because the agent is waiting on the user."""
        return self.code == "agent_blocked"


@dataclass(frozen=True)
class AgentTarget:
    """One live agent herdr can see."""

    target: str  # pane id, e.g. "w7:p1" — what `prompt` accepts
    agent: str  # "omp", "claude", ...
    status: str  # one of AGENT_STATES
    cwd: str | None = None
    title: str | None = None
    focused: bool = False

    @property
    def ready(self) -> bool:
        """idle and done both mean "ready for input" (herdr's own wording)."""
        return self.status in ("idle", "done")


def herdr_binary() -> str | None:
    """Absolute path to the herdr CLI, or None when it isn't installed."""
    found = shutil.which("herdr")
    if found:
        return found
    for candidate in _FALLBACK_BINARIES:
        path = Path(candidate).expanduser()
        if path.is_file():
            return str(path)
    return None


def available() -> bool:
    """Whether agent delivery is possible on this machine at all."""
    return herdr_binary() is not None


def _run(*args: str, timeout: float) -> dict:
    """Run a herdr command and return its JSON envelope.

    Raises VoiceTargetError for a missing binary, a timeout, a non-zero exit,
    or an ``error`` object in the response — the caller gets a reason it can
    show the user instead of a silent no-op.
    """
    binary = herdr_binary()
    if binary is None:
        raise VoiceTargetError(
            "herdr is not installed, so Voicebox cannot reach an agent session directly. "
            "Dictation will paste into the focused field instead.",
            code="herdr_missing",
        )

    try:
        result = subprocess.run(  # noqa: S603 - fixed argv, no shell
            [binary, *args],
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
    except subprocess.TimeoutExpired as exc:
        raise VoiceTargetError(
            f"herdr did not answer within {timeout:.0f}s", code="herdr_timeout"
        ) from exc

    stdout = (result.stdout or "").strip()
    payload: dict | None = None
    if stdout:
        try:
            payload = json.loads(stdout)
        except json.JSONDecodeError:
            payload = None

    if payload is not None:
        error = payload.get("error")
        if isinstance(error, dict):
            raise VoiceTargetError(
                str(error.get("message") or "herdr refused the request"),
                code=str(error.get("code") or "herdr_error"),
            )

    if result.returncode != 0:
        detail = (result.stderr or stdout or "").strip().splitlines()
        raise VoiceTargetError(
            detail[0] if detail else f"herdr exited {result.returncode}",
            code="herdr_failed",
        )

    if payload is None:
        raise VoiceTargetError("herdr returned no JSON", code="herdr_no_output")

    return payload


def list_agents() -> list[AgentTarget]:
    """Every live agent herdr can see, in its own order."""
    payload = _run("agent", "list", timeout=_LIST_TIMEOUT_SECONDS)
    agents = (payload.get("result") or {}).get("agents") or []

    targets: list[AgentTarget] = []
    for row in agents:
        pane_id = row.get("pane_id")
        if not pane_id:
            continue
        status = str(row.get("agent_status") or "unknown")
        targets.append(
            AgentTarget(
                target=str(pane_id),
                agent=str(row.get("agent") or "agent"),
                status=status if status in AGENT_STATES else "unknown",
                cwd=row.get("cwd") or row.get("foreground_cwd"),
                title=row.get("terminal_title_stripped") or row.get("terminal_title"),
                focused=bool(row.get("focused")),
            )
        )
    return targets


def send_prompt(target: str, text: str, *, voice_turn: bool = True) -> None:
    """Deliver *text* to the agent in *target*.

    A voice turn is prefixed with the marker so the agent knows to answer
    aloud; herdr validates that the target is really an agent pane before any
    input is sent, so a wrong target fails instead of typing into a shell.
    """
    if not target:
        raise VoiceTargetError("No agent target is set", code="no_target")
    if not text.strip():
        raise VoiceTargetError("Nothing to send", code="empty_text")

    body = f"{VOICE_TURN_INSTRUCTION} {text}" if voice_turn else text
    _run("agent", "prompt", target, body, timeout=_PROMPT_TIMEOUT_SECONDS)
