"""Stateless narration synthesis, served by a narration-role instance.

The main backend spawns one of these so narration can synthesize on its own model
instance while a generation occupies the main one. Two processes each have their
own MPS context, which is safe; two *threads* sharing one model are not (they
abort the process with a Metal assertion).

Deliberately stateless: the caller owns the narration session and the pill-facing
SSE contract. This endpoint only turns (profile, text) into streamed chunks.
"""

from __future__ import annotations

import asyncio
import base64
import json
import logging

from fastapi import APIRouter
from sse_starlette.sse import EventSourceResponse

from .. import models
from ..services import narration

logger = logging.getLogger(__name__)

router = APIRouter()

# Detached synthesis tasks outlive the SSE response that started them (a client
# disconnect cancels the response, not the worker thread inside the model).
_detached_synthesis: set[asyncio.Task] = set()


@router.post("/narration/warm")
async def warm():
    """Load the narration engine's model so the first line is not slow.

    The main backend calls this once the worker is healthy. Best-effort by
    design: nothing here is required for synthesis, it only moves a model load
    off the user's first narration. Resolves the engine exactly as the real
    narration path does, so it warms the model that path will ask for; no-ops
    when narration is off or no playback voice resolves.
    """
    from ..backends import engine_has_model_sizes, load_engine_model
    from ..database import get_db
    from ..services import settings as settings_service
    from ..services.profiles import get_profile_orm_by_name_or_id

    db = next(get_db())
    try:
        voice_id = settings_service.get_capture_settings(db).default_playback_voice_id
        if not voice_id:
            return {"warmed": False, "reason": "no playback voice configured"}
        profile = get_profile_orm_by_name_or_id(voice_id, db)
        if profile is None:
            return {"warmed": False, "reason": "playback voice no longer exists"}
        lane = narration.resolve_narration_lane(db, profile, None)
        if lane is None:
            return {"warmed": False, "reason": "narration lane unavailable"}
        engine, _reason = lane
    finally:
        db.close()

    size = "1.7B" if engine_has_model_sizes(engine) else "default"
    try:
        from ..backends import get_tts_backend_for_engine

        backend = get_tts_backend_for_engine(engine)
        # Never let a startup warm-up trigger a model download — mirrors
        # backends.ensure_model_cached_or_raise.
        cached = backend._is_model_cached(size) if engine_has_model_sizes(engine) else backend._is_model_cached()
        if not cached:
            return {"warmed": False, "engine": engine, "reason": "model not cached"}
        # Hold the same lock the synthesis path holds: LuxTTS/Kokoro's load_model
        # has no lock of its own, so a narration arriving during the warm-up would
        # start a second load of the same model.
        async with narration.engine_stream_lock(engine):
            await load_engine_model(engine, size)
    except Exception as exc:
        logger.exception("Narration warm-up failed")
        return {"warmed": False, "engine": engine, "reason": str(exc)}
    logger.info("Narration worker warmed %s", engine)
    return {"warmed": True, "engine": engine, "model_size": size}


@router.post("/narration/synthesize")
async def synthesize(data: models.NarrationSynthesizeRequest):
    """Stream a narration as base64 WAV chunks, synthesizing immediately."""
    queue: asyncio.Queue = asyncio.Queue()

    async def event_stream():
        producer = asyncio.create_task(
            narration.run_synthesis(
                queue,
                profile_id=data.profile_id,
                text=data.text,
                language=data.language,
                engine=data.engine,
                model_size=data.model_size,
                # The worker owns its engine instance, so there is no queue to
                # wait for — that is the entire reason it exists.
                wait_for_queue=False,
            )
        )
        _detached_synthesis.add(producer)
        producer.add_done_callback(_detached_synthesis.discard)

        chunks = 0
        try:
            yield {
                "event": "ready",
                "data": json.dumps(
                    {"narration_id": data.narration_id, "engine": data.engine}
                ),
            }
            while True:
                item = await queue.get()
                if item is None:
                    break

                kind, payload = item
                if kind == "loading":
                    yield {"event": "loading", "data": "{}"}
                elif kind == "mark":
                    # The main server owns the turn's trace; this process only
                    # reports the sub-stage so its relay can mark it there.
                    yield {"event": "mark", "data": json.dumps({"stage": payload})}
                elif kind == "chunk":
                    index, sample_rate, chunk_text, wav_bytes = payload
                    chunks += 1
                    yield {
                        "event": "chunk",
                        "data": json.dumps(
                            {
                                "index": index,
                                "sample_rate": sample_rate,
                                "text": chunk_text,
                                "wav": base64.b64encode(wav_bytes).decode("ascii"),
                            }
                        ),
                    }
                elif kind == "error":
                    raise payload

            yield {"event": "done", "data": json.dumps({"chunks": chunks})}
        except asyncio.CancelledError:
            raise
        except Exception as exc:
            logger.exception("Narration worker synthesis failed")
            yield {"event": "error", "data": json.dumps({"message": str(exc)})}

    return EventSourceResponse(event_stream())
