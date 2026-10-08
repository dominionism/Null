"""The contract between Null and an agent harness.

A *harness* is an agent CLI the user already has installed and signed in to:
Oh-my-pi, OpenCode, Claude Code, Codex. Null starts it and talks to it; the
harness keeps its own sign-in, tools, MCP servers and transcript. Nothing in
this package reads, stores or forwards a provider token.

Everything above this layer sees only the `Harness` protocol and the events
below, never a harness's own transport. An adapter that needs a side channel
to report something means the event set here is too thin, and should grow.
"""

from __future__ import annotations

from collections.abc import AsyncIterator
from dataclasses import asdict, dataclass
from datetime import datetime
from typing import Any, Literal, Protocol, runtime_checkable

#: What a session is doing, in the mini's own words. Adapters map their
#: harness's states onto these four.
SessionStatus = Literal["running", "needs_input", "ready", "blocked"]

#: How one reply ended. A failure is a `HarnessError` event, not a stop reason.
StopReason = Literal["completed", "cancelled", "truncated", "refused"]

ToolStatus = Literal["pending", "in_progress", "completed", "failed"]

ApprovalKind = Literal["allow_once", "allow_always", "reject_once", "reject_always"]


class HarnessUnavailableError(RuntimeError):
    """The harness cannot be started or reached: not installed, wrong version, exited."""


@dataclass(frozen=True)
class TextDelta:
    """A piece of the reply as it streams."""

    text: str
    thinking: bool = False  # reasoning text, which harnesses stream apart from the answer


@dataclass(frozen=True)
class MessageDone:
    """The harness finished answering one `send`."""

    stop_reason: StopReason = "completed"


@dataclass(frozen=True)
class ToolActivity:
    """A tool call the harness made, reported again each time its status changes."""

    call_id: str
    title: str
    status: ToolStatus
    kind: str | None = None  # the harness's own category: "read", "edit", "execute", ...
    raw_input: Any = None
    raw_output: Any = None


@dataclass(frozen=True)
class ApprovalOption:
    """One answer the harness will accept to an approval request."""

    option_id: str
    label: str
    kind: ApprovalKind


@dataclass(frozen=True)
class ApprovalRequest:
    """The harness is waiting on the user before it goes on. Answer with `respond`."""

    request_id: str
    title: str
    options: tuple[ApprovalOption, ...] = ()  # empty when the harness wants free text
    call_id: str | None = None  # the tool call this gates, when there is one


@dataclass(frozen=True)
class StatusChange:
    """The session moved to a new status."""

    status: SessionStatus
    reason: str | None = None


@dataclass(frozen=True)
class LimitReached:
    """The provider refused the request because a usage limit is used up."""

    message: str
    window: str | None = None  # e.g. "5h" or "weekly", when the harness says
    resets_at: datetime | None = None


@dataclass(frozen=True)
class HarnessError:
    """Something failed inside the harness or its provider. An event, not an exception."""

    message: str
    code: int | str | None = None
    details: str | None = None


HarnessEvent = TextDelta | MessageDone | ToolActivity | ApprovalRequest | StatusChange | LimitReached | HarnessError

#: Wire name of each event, for anything that sends events across a process boundary.
_EVENT_TYPES: dict[type, str] = {
    TextDelta: "text_delta",
    MessageDone: "message_done",
    ToolActivity: "tool_activity",
    ApprovalRequest: "approval_request",
    StatusChange: "status_change",
    LimitReached: "limit_reached",
    HarnessError: "error",
}


def event_to_dict(event: HarnessEvent) -> dict[str, Any]:
    """Flatten an event to JSON-ready data, tagged with its `type`."""
    data = asdict(event)
    if isinstance(event, LimitReached) and event.resets_at is not None:
        data["resets_at"] = event.resets_at.isoformat()
    return {"type": _EVENT_TYPES[type(event)], **data}


@dataclass(frozen=True)
class HarnessInfo:
    """Whether a harness can be used right now, and how to fix it if not."""

    name: str  # registry key, e.g. "omp"
    display_name: str  # e.g. "Oh-my-pi"
    installed: bool
    binary: str | None = None
    version: str | None = None
    signed_in: bool | None = None  # None when the harness cannot say without a session
    fix_command: str | None = None  # e.g. "omp login"


@dataclass(frozen=True)
class ModelChoice:
    """One model the harness can reach with the sign-ins it holds."""

    id: str  # what `set_model` accepts, e.g. "opencode-go/deepseek-v4.1-flash"
    label: str
    provider: str | None = None


@dataclass(frozen=True)
class HarnessSession:
    """A conversation the harness owns. Null keeps the id, the harness keeps the transcript."""

    session_id: str
    cwd: str
    model: str | None = None


@runtime_checkable
class Harness(Protocol):
    """One installed agent CLI, driven headlessly. One instance serves many sessions."""

    async def info(self) -> HarnessInfo:
        """Report whether the harness is installed and signed in."""
        ...

    async def start_session(
        self,
        cwd: str,
        *,
        resume: str | None = None,
        model: str | None = None,
    ) -> HarnessSession:
        """Open a session in `cwd`, or reopen the harness's own session `resume`."""
        ...

    def send(self, session_id: str, text: str) -> AsyncIterator[HarnessEvent]:
        """Send one prompt and yield what happens, ending with `MessageDone` or `HarnessError`."""
        ...

    async def respond(self, session_id: str, request_id: str, answer: str) -> None:
        """Answer an `ApprovalRequest`: an option id, or free text when it offered none."""
        ...

    async def interrupt(self, session_id: str) -> None:
        """Stop the reply in progress. The session stays usable."""
        ...

    async def list_models(self, session_id: str | None = None) -> list[ModelChoice]:
        """List the models the harness can reach, for a session or in general."""
        ...

    async def set_model(self, session_id: str, model_id: str) -> None:
        """Switch a session to another model. The conversation carries on."""
        ...

    async def close(self) -> None:
        """Stop the harness process and release everything it holds."""
        ...
