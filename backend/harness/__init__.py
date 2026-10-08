"""Agent harnesses Null can drive, and the registry that hands them out.

Mirrors `backends/`: callers ask for a harness by name and get one shared
instance, created on first use. An adapter is imported inside its factory, so
a harness nobody uses costs nothing at startup.
"""

from __future__ import annotations

import logging
import threading
from collections.abc import Callable
from dataclasses import dataclass

from .base import (
    ApprovalOption,
    ApprovalRequest,
    Harness,
    HarnessError,
    HarnessEvent,
    HarnessInfo,
    HarnessSession,
    HarnessUnavailableError,
    LimitReached,
    MessageDone,
    ModelChoice,
    SessionStatus,
    StatusChange,
    TextDelta,
    ToolActivity,
    event_to_dict,
)
from .discovery import find_binary

logger = logging.getLogger(__name__)

__all__ = [
    "HARNESSES",
    "ApprovalOption",
    "ApprovalRequest",
    "Harness",
    "HarnessError",
    "HarnessEvent",
    "HarnessInfo",
    "HarnessSession",
    "HarnessSpec",
    "HarnessUnavailableError",
    "LimitReached",
    "MessageDone",
    "ModelChoice",
    "SessionStatus",
    "StatusChange",
    "TextDelta",
    "ToolActivity",
    "close_harnesses",
    "event_to_dict",
    "find_binary",
    "get_harness",
    "harness_names",
    "reset_harnesses",
]


@dataclass(frozen=True)
class HarnessSpec:
    """One row of the registry."""

    name: str  # registry key, e.g. "omp"
    display_name: str  # e.g. "Oh-my-pi"
    factory: Callable[[], Harness]


def _create_omp() -> Harness:
    from .acp import ACP_AGENTS, AcpHarness

    return AcpHarness(ACP_AGENTS["omp"])


#: Supported harnesses, keyed by name.
HARNESSES: dict[str, HarnessSpec] = {
    "omp": HarnessSpec("omp", "Oh-my-pi", _create_omp),
}

_harnesses: dict[str, Harness] = {}
_harnesses_lock = threading.Lock()


def harness_names() -> list[str]:
    """Names of every supported harness."""
    return list(HARNESSES)


def get_harness(name: str) -> Harness:
    """Get or create the shared instance of a harness."""
    # Fast path: check without lock
    if name in _harnesses:
        return _harnesses[name]

    # Slow path: create with lock to avoid duplicate instantiation
    with _harnesses_lock:
        if name in _harnesses:
            return _harnesses[name]

        spec = HARNESSES.get(name)
        if spec is None:
            raise ValueError(f"Unknown harness: {name}. Supported: {list(HARNESSES.keys())}")

        harness = spec.factory()
        _harnesses[name] = harness
        return harness


def reset_harnesses() -> None:
    """Drop created instances (useful for testing). The rows in `HARNESSES` stay."""
    with _harnesses_lock:
        _harnesses.clear()


async def close_harnesses() -> None:
    """Stop every harness that was started. Called when the server exits."""
    with _harnesses_lock:
        started = dict(_harnesses)
        _harnesses.clear()
    for name, harness in started.items():
        try:
            await harness.close()
        except Exception:
            logger.exception("Could not stop harness %s", name)
