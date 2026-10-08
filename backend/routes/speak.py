"""Speak endpoints — REST wrappers around ``voicebox.speak`` for non-MCP callers.

``POST /speak`` plays text through a profile on the normal (queued, persisted)
generation path. ``POST /speak/narrate`` is the agent-narration lane: CPU-only,
queue-independent, streamed over ``GET /speak/{narration_id}/stream`` and never
persisted to the generations table.

Shell scripts, ACP, A2A, or any agent that doesn't speak MCP can hit these
endpoints. Profile resolution and per-client bindings
(``X-Voicebox-Client-Id``) behave identically to the MCP tool.
"""

from __future__ import annotations

import asyncio
import base64
import json
import logging
import uuid
from dataclasses import dataclass
from typing import Any

import httpx
from fastapi import APIRouter, Depends, HTTPException, Request
from sqlalchemy.orm import Session
from sse_starlette.sse import EventSourceResponse

from .. import models
from ..database import MCPClientBinding, get_db
from ..mcp_server import events as mcp_events
from ..mcp_server.resolve import resolve_profile
from ..services.settings import agent_voice_enabled
from ..services import narration, narration_worker
from ..utils import timing

logger = logging.getLogger(__name__)

# Written for the *agent* as much as the user: an MCP tool call that fails with
# this message tells the model to stop trying rather than retry.
AGENT_VOICE_OFF = (
    "Agent voice is off — the user muted agent speech. Do not retry; keep working "
    "silently and wait until they turn it back on."
)

router = APIRouter()

# Detached narration synthesis tasks outlive the SSE response that started them:
# a client disconnect cancels the response, not the worker thread already inside
# the model. Holding a reference keeps them alive (and observable) until they
# finish and release the engine lock.
_detached_synthesis: set[asyncio.Task] = set()


@dataclass
class SpeakContext:
    """Resolved profile, per-client binding defaults, and engine choice."""

    profile: Any
    client_id: str | None
    personality_flag: bool
    engine: str | None


def _resolve_speak_context(
    data: models.SpeakRequest,
    request: Request,
    db: Session,
) -> SpeakContext:
    """Resolve the profile, per-client binding defaults, and engine for a speak call."""
    client_id = request.headers.get("X-Voicebox-Client-Id")
    profile = resolve_profile(data.profile, client_id, db)
    if profile is None:
        if data.profile:
            raise HTTPException(
                status_code=404,
                detail=f"Voice profile '{data.profile}' not found.",
            )
        raise HTTPException(
            status_code=400,
            detail=(
                "No voice profile resolved. Pass `profile` (name or id), "
                "or configure a default in Voicebox → Settings → MCP."
            ),
        )

    binding = None
    if client_id:
        binding = (
            db.query(MCPClientBinding)
            .filter(MCPClientBinding.client_id == client_id)
            .first()
        )

    # Resolve per-client personality default when the caller didn't pin it.
    personality_flag = data.personality
    if personality_flag is None and binding is not None:
        personality_flag = bool(binding.default_personality)

    engine = data.engine
    if engine is None and binding is not None:
        engine = binding.default_engine

    return SpeakContext(
        profile=profile,
        client_id=client_id,
        personality_flag=bool(personality_flag),
        engine=engine,
    )


async def narrate_speech(
    *,
    db: Session,
    context: SpeakContext,
    text: str,
    language: str,
    source: str,
    narrate: bool,
    model_size: str | None = None,
) -> models.NarrateSpeakResponse:
    """
    Speak ``text`` on the narration lane, or degrade to the queued speak path.

    The narration lane is used when the profile has a CPU narration voice (see
    :func:`services.narration.resolve_narration_lane`) and produces an ephemeral
    session the caller streams from ``GET /speak/{narration_id}/stream``.
    Otherwise the request takes the normal queued, persisted path and the
    response carries ``mode="generation"``.
    """
    profile = context.profile
    lane = narration.resolve_narration_lane(db, profile, context.engine) if narrate else None

    if lane is None:
        # Degrade to the ordinary speak path — same payload the queued route
        # publishes, so the pill's non-narration branch handles it untouched.
        from .generations import generate_speech

        generation = await generate_speech(
            models.GenerationRequest(
                profile_id=profile.id,
                text=text,
                language=language,
                engine=context.engine,
                personality=context.personality_flag,
                model_size=model_size or "1.7B",
            ),
            db,
            source=source,
        )
        generation_id = getattr(generation, "id", None)
        mcp_events.publish(
            "speak-start",
            {
                "generation_id": generation_id,
                "profile_name": profile.name,
                "source": source,
                "client_id": context.client_id,
            },
        )
        return models.NarrateSpeakResponse(
            mode="generation",
            generation_id=generation_id,
            poll_url=f"/generate/{generation_id}/status" if generation_id else None,
            status=getattr(generation, "status", None),
            profile=profile.name,
            engine=context.engine or getattr(profile, "default_engine", None),
        )

    engine, reason = lane

    # The narration id doubles as the voice-loop trace id: the pill adopts it
    # from speak-start, and this process marks the whole synthesis half under it.
    turn_id = str(uuid.uuid4())
    timing.mark(turn_id, "speak_request")

    # Rewrite before the session is created: the personality LLM must stay off
    # the streaming path, and the stream carries only final text.
    resolved_text = text
    if context.personality_flag and getattr(profile, "personality", None):
        from ..services import personality

        try:
            llm_result = await personality.rewrite_as_profile(profile.personality, text)
        except ValueError as e:
            raise HTTPException(status_code=400, detail=str(e)) from e
        resolved_text = llm_result.text.strip()
        if not resolved_text:
            raise HTTPException(
                status_code=500,
                detail="LLM produced empty output; nothing to speak.",
            )

    session = narration.create_session(
        text=resolved_text,
        profile_id=profile.id,
        profile_name=profile.name,
        engine=engine,
        language=language,
        reason=reason,
        model_size=model_size,
        session_id=turn_id,
    )
    logger.info(
        "Narration %s queued for streaming (engine=%s, reason=%s, %d chars, source=%s)",
        session.id,
        engine,
        reason,
        len(resolved_text),
        source,
    )
    mcp_events.publish(
        "speak-start",
        {
            "generation_id": session.id,
            "narration": True,
            "profile_name": profile.name,
            "source": source,
            "client_id": context.client_id,
        },
    )
    return models.NarrateSpeakResponse(
        mode="narration",
        narration_id=session.id,
        stream_url=f"/speak/{session.id}/stream",
        profile=profile.name,
        engine=engine,
    )


@router.post("/speak", response_model=models.GenerationResponse)
async def speak(
    data: models.SpeakRequest,
    request: Request,
    db: Session = Depends(get_db),
):
    """Speak text in a voice profile. Mirrors voicebox.speak (MCP).

    Response shape matches POST /generate — a ``GenerationResponse`` with
    ``status="generating"`` and an ``id`` the caller polls at
    ``GET /generate/{id}/status``.
    """
    if not agent_voice_enabled(db):
        raise HTTPException(status_code=409, detail=AGENT_VOICE_OFF)

    context = _resolve_speak_context(data, request, db)

    from .generations import generate_speech

    generation = await generate_speech(
        models.GenerationRequest(
            profile_id=context.profile.id,
            text=data.text,
            language=data.language or "en",
            engine=context.engine,
            personality=context.personality_flag,
        ),
        db,
        source="rest",
    )

    mcp_events.publish(
        "speak-start",
        {
            "generation_id": getattr(generation, "id", None),
            "profile_name": context.profile.name,
            "source": "rest",
            "client_id": context.client_id,
        },
    )
    return generation


@router.post("/speak/narrate", response_model=models.NarrateSpeakResponse)
async def narrate(
    data: models.SpeakRequest,
    request: Request,
    db: Session = Depends(get_db),
):
    """Speak text as live agent narration on the CPU lane.

    Never blocks behind a running generation and never writes to history.
    Falls back to the queued speak path (``mode="generation"``) when the profile
    has no CPU narration voice — see ``services/narration.resolve_narration_lane``.
    """
    if not agent_voice_enabled(db):
        raise HTTPException(status_code=409, detail=AGENT_VOICE_OFF)

    context = _resolve_speak_context(data, request, db)
    return await narrate_speech(
        db=db,
        context=context,
        text=data.text,
        language=data.language or "en",
        source="rest",
        narrate=True,
    )


@router.post("/speak/stop")
async def stop_speaking(db: Session = Depends(get_db)):
    """
    Silence agent speech now — all of it.

    Cancels every live narration (a running stream stops at its next chunk
    boundary and closes the worker connection), kills the headless player if it
    is mid-file, and publishes ``speak-end``/``cancelled`` so the pill dismisses
    and drops whatever audio it has buffered. The user's chord calls this.
    """
    from ..services import narration, playback

    cancelled = narration.cancel_all()
    killed = playback.stop_playback()
    mcp_events.publish("speak-end", {"generation_id": None, "status": "cancelled"})
    logger.info(
        "Agent speech stopped: %d narration(s) cancelled, headless player killed=%s",
        cancelled,
        killed,
    )
    return {"narrations_cancelled": cancelled, "playback_stopped": killed}


async def _iter_sse(response):
    """Yield ``(event, data)`` for each SSE frame in a streaming response."""
    event = "message"
    data: list[str] = []
    async for line in response.aiter_lines():
        if line.startswith("event:"):
            event = line[len("event:") :].strip()
        elif line.startswith("data:"):
            data.append(line[len("data:") :].strip())
        elif not line.strip() and data:
            yield event, "\n".join(data)
            event, data = "message", []


@router.get("/speak/{narration_id}/stream")
async def narration_stream(narration_id: str):
    """
    Stream a narration's audio chunk by chunk as it is synthesized.

    Events: ``ready`` (session metadata), ``loading`` (model load/download
    starting), ``chunk`` (``index``, ``sample_rate``, ``text``, base64 ``wav``),
    ``done`` (``chunks``), or ``error`` (``message``).

    The stream's lifetime *is* the narration's lifetime: disconnecting cancels
    synthesis (at the next chunk boundary), discards the session, and publishes
    ``speak-end`` with ``status="cancelled"``. Nothing is persisted.
    """
    session = narration.get_session(narration_id)
    if session is None:
        raise HTTPException(status_code=404, detail="Narration not found.")
    if narration.claim_session(narration_id) is None:
        raise HTTPException(status_code=409, detail="Narration is already being streamed.")

    async def event_stream():
        status = "completed"
        chunks = 0
        queue: asyncio.Queue = asyncio.Queue()

        async def relay_from_worker(base_url: str) -> None:
            nonlocal status
            """
            Relay the narration worker's chunk stream onto the queue.

            The worker owns its own model instance, so this path has no queue
            gate — that is the entire reason it exists: narration can synthesize
            while a generation holds this process's engine.
            """
            payload = {
                "narration_id": session.id,
                "profile_id": session.profile_id,
                "text": session.text,
                "language": session.language,
                "engine": session.engine,
                "model_size": session.model_size,
            }
            try:
                async with httpx.AsyncClient(timeout=httpx.Timeout(None)) as client, client.stream(
                    "POST", f"{base_url}/narration/synthesize", json=payload
                ) as response:
                    if response.status_code != 200:
                        raise RuntimeError(
                            f"narration worker returned {response.status_code}"
                        )
                    async for event, raw in _iter_sse(response):
                        if event == "chunk":
                            if session.cancelled:
                                # Closing the response cancels the worker's
                                # synthesis at its next chunk boundary.
                                status = "cancelled"
                                break
                            chunk = json.loads(raw)
                            await queue.put(
                                (
                                    "chunk",
                                    (
                                        chunk["index"],
                                        chunk["sample_rate"],
                                        chunk["text"],
                                        base64.b64decode(chunk["wav"]),
                                    ),
                                )
                            )
                        elif event == "loading":
                            await queue.put(("loading", None))
                        elif event == "mark":
                            await queue.put(("mark", json.loads(raw).get("stage", "")))
                        elif event == "error":
                            await queue.put(
                                ("error", RuntimeError(json.loads(raw).get("message", "")))
                            )
                            # `ready` and `done` are this server's to emit.
            except Exception as exc:
                await queue.put(("error", exc))
            finally:
                await queue.put(None)

        async def synthesize() -> None:
            """
            Run the in-process synthesis loop onto the queue.

            Kept in its own task so a client disconnect cannot release the engine
            lock while the worker thread is still inside the model: cancelling
            the await does not stop that thread, and the next narration would
            then drive the same model from a second thread, which aborts the
            process (`_status < MTLCommandBufferStatusCommitted`).
            """
            await narration.run_synthesis(
                queue,
                profile_id=session.profile_id,
                text=session.text,
                language=session.language,
                engine=session.engine,
                model_size=session.model_size,
                wait_for_queue=True,
                cancel_check=lambda: session.cancelled,
            )

        try:
            timing.mark(session.id, "stream_open")
            yield {
                "event": "ready",
                "data": json.dumps(
                    {
                        "narration_id": session.id,
                        "profile_name": session.profile_name,
                        "engine": session.engine,
                        "language": session.language,
                        "text": session.text,
                    }
                ),
            }

            worker = narration_worker.worker_base_url()
            if worker:
                logger.info("Narration %s handed to the worker at %s", session.id, worker)
            producer = asyncio.create_task(
                relay_from_worker(worker) if worker else synthesize()
            )
            # A detached producer outlives this response; keep a reference so it
            # cannot be garbage collected while its worker thread is running.
            _detached_synthesis.add(producer)
            producer.add_done_callback(_detached_synthesis.discard)

            while True:
                item = await queue.get()
                if item is None:
                    break

                kind, payload = item
                if kind == "waiting":
                    yield {
                        "event": "waiting",
                        "data": json.dumps({"waited_ms": payload}),
                    }
                elif kind == "loading":
                    yield {"event": "loading", "data": "{}"}
                elif kind == "mark":
                    timing.mark(session.id, payload)
                elif kind == "chunk":
                    index, sample_rate, chunk_text, wav_bytes = payload
                    chunks += 1
                    if chunks == 1:
                        timing.mark(session.id, "first_chunk")
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
                else:
                    raise payload

            timing.mark(session.id, "last_chunk")
            yield {"event": "done", "data": json.dumps({"chunks": chunks})}
        except asyncio.CancelledError:
            # Client disconnected — the narration is cancelled, not failed.
            status = "cancelled"
            raise
        except Exception as e:
            status = "failed"
            logger.exception("Narration %s failed", narration_id)
            yield {"event": "error", "data": json.dumps({"message": str(e)})}
        finally:
            timing.finish_turn(session.id)
            narration.discard_session(narration_id)
            mcp_events.publish(
                "speak-end",
                {"generation_id": narration_id, "status": status},
            )

    return EventSourceResponse(event_stream())
