"""Turn-trace endpoints — dev instrumentation for the voice loop.

See :mod:`backend.utils.timing` and ``Context/Plans/VoiceLoopLatency.md``.
Everything here is best-effort: it reports what it recorded and never raises
for a turn it has not seen.
"""

from __future__ import annotations

from fastapi import APIRouter

from .. import models
from ..utils import timing

router = APIRouter(tags=["turns"])


@router.post("/turns/{turn_id}/begin")
async def begin_turn(turn_id: str, request: models.TurnBeginRequest | None = None):
    """Anchor a turn. The pill calls this at chord release."""
    anchor = request.anchor_epoch_ms if request is not None else None
    trace = timing.begin_turn(turn_id, anchor)
    return {"turn_id": trace.turn_id, "enabled": timing.is_enabled()}


@router.post("/turns/{turn_id}/marks")
async def mark_turn(turn_id: str, request: models.TurnMarkRequest):
    """Record one stage. The first write for a stage wins, so duplicates are free."""
    recorded = timing.mark(turn_id, request.stage, **(request.meta or {}))
    return {"turn_id": turn_id, "stage": request.stage, "recorded": recorded}


@router.get("/turns/latest")
async def latest_turns(limit: int = 20):
    """Recent turns, most recent first."""
    return {"turns": [trace.as_dict() for trace in timing.latest(limit)]}


@router.delete("/turns")
async def clear_turns():
    """Drop the in-memory traces (``turns.jsonl`` on disk is left alone)."""
    return {"cleared": timing.clear()}
