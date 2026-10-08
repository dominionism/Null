"""Mini sessions: one conversation between the user and a harness.

The harness keeps the transcript. Null keeps the list of open sessions and, for
each, the events of its replies, so a window that was hidden or lost its
connection can catch up from where it stopped.

A reply runs as a background task, not inside the request that asked for it:
hiding the mini must not stop work the user asked for.

Sessions live in memory for now. A server restart forgets them; the harness
still has the conversation.
"""

from __future__ import annotations

import asyncio
import logging
from collections import deque
from collections.abc import AsyncIterator
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from .. import config
from ..harness import HarnessError, SessionStatus, StatusChange, close_harnesses, event_to_dict, get_harness

logger = logging.getLogger(__name__)

DEFAULT_HARNESS = "omp"

#: Events kept per session for catching up. A long reply is a few hundred.
_MAX_EVENTS = 5000


class MiniSessionNotFoundError(LookupError):
    """No open session has this id."""


class MiniBusyError(RuntimeError):
    """The session is still answering the previous message."""


@dataclass
class MiniSession:
    """One open conversation. `id` is the harness's own session id."""

    id: str
    harness: str
    cwd: str
    model: str | None = None
    status: SessionStatus = "ready"
    busy: bool = False
    last_seq: int = 0
    events: deque[tuple[int, dict[str, Any]]] = field(default_factory=lambda: deque(maxlen=_MAX_EVENTS))
    changed: asyncio.Condition = field(default_factory=asyncio.Condition)
    task: asyncio.Task[None] | None = None


_sessions: dict[str, MiniSession] = {}


def default_workspace() -> Path:
    """Where a session works when the caller names no directory."""
    return config.get_data_dir() / "mini" / "workspace"


def get_session(session_id: str) -> MiniSession:
    session = _sessions.get(session_id)
    if session is None:
        raise MiniSessionNotFoundError(session_id)
    return session


async def create_session(harness: str | None = None, cwd: str | None = None) -> MiniSession:
    """Open a session on `harness`, working in `cwd`. Both fall back to the defaults."""
    harness = harness or DEFAULT_HARNESS
    if cwd:
        workspace = Path(cwd).expanduser()
        if not workspace.is_dir():
            raise ValueError(f"Not a directory: {workspace}")
    else:
        workspace = default_workspace()
        workspace.mkdir(parents=True, exist_ok=True)

    started = await get_harness(harness).start_session(str(workspace))
    session = MiniSession(id=started.session_id, harness=harness, cwd=started.cwd, model=started.model)
    _sessions[session.id] = session
    return session


async def send_message(session_id: str, text: str) -> int:
    """Start a reply to `text`. Returns the cursor to read its events after."""
    session = get_session(session_id)
    if session.busy:
        raise MiniBusyError(session_id)
    cursor = session.last_seq
    session.busy = True
    await _publish(session, {"type": "user_message", "text": text})
    session.task = asyncio.create_task(_run_reply(session, text))
    return cursor


async def stream_events(
    session_id: str, after: int = 0, heartbeat: float = 15.0
) -> AsyncIterator[tuple[int, dict[str, Any] | None]]:
    """Yield `(seq, event)` for every event after `after`, until the session is idle and caught up.

    Yields `(cursor, None)` when `heartbeat` seconds pass with nothing new, so
    the caller can keep a quiet connection alive through a long tool call.
    """
    session = get_session(session_id)
    cursor = after
    while True:
        fresh = [item for item in session.events if item[0] > cursor]
        for seq, event in fresh:
            cursor = seq
            yield seq, event
        if fresh:
            continue
        if not session.busy:
            return
        async with session.changed:
            try:
                await asyncio.wait_for(
                    session.changed.wait_for(lambda seen=cursor: session.last_seq > seen or not session.busy),
                    timeout=heartbeat,
                )
            except TimeoutError:
                yield cursor, None


async def respond(session_id: str, request_id: str, answer: str) -> None:
    """Answer an approval request the harness is waiting on."""
    session = get_session(session_id)
    await get_harness(session.harness).respond(session.id, request_id, answer)


async def interrupt(session_id: str) -> None:
    """Stop the reply in progress. The session stays open."""
    session = get_session(session_id)
    await get_harness(session.harness).interrupt(session.id)


async def shutdown() -> None:
    """Stop every reply and every harness process. Called when the server exits."""
    for session in _sessions.values():
        if session.task is not None:
            session.task.cancel()
    _sessions.clear()
    await close_harnesses()


async def _publish(session: MiniSession, event: dict[str, Any]) -> None:
    async with session.changed:
        session.last_seq += 1
        session.events.append((session.last_seq, event))
        session.changed.notify_all()


async def _run_reply(session: MiniSession, text: str) -> None:
    try:
        async for event in get_harness(session.harness).send(session.id, text):
            if isinstance(event, StatusChange):
                session.status = event.status
            await _publish(session, event_to_dict(event))
    except Exception as exc:
        logger.exception("Mini session %s: the reply failed", session.id)
        session.status = "blocked"
        await _publish(session, event_to_dict(HarnessError(message=str(exc) or type(exc).__name__)))
    finally:
        session.busy = False
        async with session.changed:
            session.changed.notify_all()
