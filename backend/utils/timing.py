"""Per-turn timing marks for the voice loop.

Dev instrumentation for ``Context/Plans/VoiceLoopLatency.md``: one record per
turn, joined across the chord and the first audible audio, so the loop's stages
can be ranked by wall-clock cost instead of guessed.

Marks are O(1) appends to an in-memory ring buffer; the only I/O is one JSON
line per finished turn under ``<data>/logs/turns.jsonl``. Set
``VOICEBOX_TRACE=0`` to switch the whole thing off.
"""

from __future__ import annotations

import json
import logging
import os
import time
from collections import OrderedDict
from dataclasses import dataclass, field

from .. import config

logger = logging.getLogger(__name__)

ENV_VAR = "VOICEBOX_TRACE"
#: Bounded so a long session cannot grow without limit. A finished turn is
#: already on disk in ``turns.jsonl`` by the time it can be evicted here.
MAX_TRACES = 50
#: The stage that closes a turn and writes it to disk.
END_STAGE = "turn_end"

_PROCESS_START = time.monotonic()


def is_enabled() -> bool:
    """Whether tracing is on (``VOICEBOX_TRACE``, default on)."""
    return os.environ.get(ENV_VAR, "1").strip().lower() not in {"0", "false", "no", "off"}


def _now_ms() -> float:
    """Monotonic milliseconds since this process started."""
    return (time.monotonic() - _PROCESS_START) * 1000.0


@dataclass
class TurnTrace:
    """One turn's marks: ``(stage, at_ms, meta)``, first write wins per stage."""

    turn_id: str
    anchor_epoch_ms: int | None = None
    started_ms: float = field(default_factory=_now_ms)
    marks: list[tuple[str, float, dict]] = field(default_factory=list)
    finished: bool = False

    def stages(self) -> set[str]:
        """Stages already recorded for this turn."""
        return {stage for stage, _at, _meta in self.marks}

    def as_dict(self) -> dict:
        """Serializable form. ``age_ms`` is measured from the turn's first mark."""
        return {
            "turn_id": self.turn_id,
            "anchor_epoch_ms": self.anchor_epoch_ms,
            "age_ms": round(_now_ms() - self.started_ms, 1),
            "finished": self.finished,
            "marks": [{"stage": stage, "at_ms": round(at_ms, 1), "meta": meta} for stage, at_ms, meta in self.marks],
        }


_traces: OrderedDict[str, TurnTrace] = OrderedDict()


def begin_turn(turn_id: str, anchor_epoch_ms: int | None = None) -> TurnTrace:
    """Start a turn, or return the one already started under this id."""
    existing = _traces.get(turn_id)
    if existing is not None:
        if anchor_epoch_ms is not None and existing.anchor_epoch_ms is None:
            existing.anchor_epoch_ms = anchor_epoch_ms
        return existing

    trace = TurnTrace(turn_id=turn_id, anchor_epoch_ms=anchor_epoch_ms)
    if is_enabled():
        _store(trace)
    return trace


def _store(trace: TurnTrace) -> None:
    _traces[trace.turn_id] = trace
    while len(_traces) > MAX_TRACES:
        _traces.popitem(last=False)


def mark(turn_id: str | None, stage: str, **meta) -> bool:
    """Record ``stage`` for a turn; False when it was already recorded.

    Creates the turn when it does not exist yet, so a mark that arrives before
    its ``begin`` is not lost. Idempotent per ``(turn_id, stage)`` — the first
    write wins, which is what lets both sides of a boundary mark the same stage
    without either having to know the other ran.
    """
    if not turn_id or not is_enabled():
        return False

    trace = _traces.get(turn_id) or begin_turn(turn_id)
    if stage in trace.stages():
        return False
    trace.marks.append((stage, _now_ms(), meta))
    if stage == END_STAGE:
        finish_turn(turn_id)
    return True


def finish_turn(turn_id: str | None) -> TurnTrace | None:
    """Close a turn and append it to ``turns.jsonl``. Writes at most once."""
    if not turn_id or not is_enabled():
        return None

    trace = _traces.get(turn_id)
    if trace is None or trace.finished:
        return trace

    trace.finished = True
    _append(trace)
    return trace


def _append(trace: TurnTrace) -> None:
    try:
        logs = config.get_data_dir() / "logs"
        logs.mkdir(parents=True, exist_ok=True)
        with (logs / "turns.jsonl").open("a", encoding="utf-8") as handle:
            handle.write(json.dumps(trace.as_dict()) + "\n")
    except OSError:
        logger.exception("Could not append the turn trace for %s", trace.turn_id)


def latest(limit: int = 20) -> list[TurnTrace]:
    """Recent turns, most recent first."""
    if not is_enabled():
        return []
    return list(reversed(list(_traces.values())))[: max(1, limit)]


def clear() -> int:
    """Drop every stored turn. Returns how many were dropped."""
    count = len(_traces)
    _traces.clear()
    return count
