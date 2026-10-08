"""The mini: a conversation with the user's own agent harness.

- ``GET /harnesses`` — which harnesses Null can drive, and whether each is installed.
- ``POST /mini/sessions`` — open a conversation.
- ``GET /mini/sessions/{id}`` — its status, and the cursor of its latest event.
- ``PATCH /mini/sessions/{id}`` — move it to another model.
- ``GET /mini/sessions/{id}/models`` — the models it can be moved to.
- ``POST /mini/sessions/{id}/messages`` — send a message. The reply runs in the
  background; read it from the events stream.
- ``GET /mini/sessions/{id}/events?after=N`` — SSE stream of everything after
  cursor ``N``. It ends when the session is idle and the reader has caught up.
- ``POST /mini/sessions/{id}/respond`` — answer an approval request.
- ``POST /mini/sessions/{id}/interrupt`` — stop the reply in progress.

Every route here is locked (see ``services/mini_auth``): these endpoints make
an agent act on this machine.
"""

from __future__ import annotations

import json
import logging

from fastapi import APIRouter, Depends, HTTPException
from sse_starlette.sse import EventSourceResponse

from .. import models
from ..harness import HarnessUnavailableError, get_harness, harness_names
from ..services import mini
from ..services.mini_auth import require_local_caller

logger = logging.getLogger(__name__)

router = APIRouter(tags=["mini"], dependencies=[Depends(require_local_caller)])


def _session_response(session: mini.MiniSession) -> models.MiniSessionResponse:
    return models.MiniSessionResponse(
        id=session.id,
        harness=session.harness,
        cwd=session.cwd,
        model=session.model,
        status=session.status,
        busy=session.busy,
        last_seq=session.last_seq,
    )


def _get_session(session_id: str) -> mini.MiniSession:
    try:
        return mini.get_session(session_id)
    except mini.MiniSessionNotFoundError as exc:
        raise HTTPException(status_code=404, detail="No open mini session has this id.") from exc


@router.get("/harnesses", response_model=list[models.HarnessInfoResponse])
async def list_harnesses() -> list[models.HarnessInfoResponse]:
    """Every harness Null can drive, with what is needed to use it."""
    infos = [await get_harness(name).info() for name in harness_names()]
    return [
        models.HarnessInfoResponse(
            name=info.name,
            display_name=info.display_name,
            installed=info.installed,
            binary=info.binary,
            version=info.version,
            signed_in=info.signed_in,
            fix_command=info.fix_command,
        )
        for info in infos
    ]


@router.post("/mini/sessions", response_model=models.MiniSessionResponse)
async def create_session(data: models.MiniSessionCreate) -> models.MiniSessionResponse:
    """Open a conversation on a harness."""
    try:
        session = await mini.create_session(harness=data.harness, cwd=data.cwd, model=data.model)
    except ValueError as exc:
        raise HTTPException(status_code=400, detail=str(exc)) from exc
    except HarnessUnavailableError as exc:
        raise HTTPException(status_code=503, detail=str(exc)) from exc
    return _session_response(session)


@router.get("/mini/sessions/{session_id}", response_model=models.MiniSessionResponse)
async def get_session(session_id: str) -> models.MiniSessionResponse:
    return _session_response(_get_session(session_id))


@router.patch("/mini/sessions/{session_id}", response_model=models.MiniSessionResponse)
async def update_session(session_id: str, data: models.MiniSessionUpdate) -> models.MiniSessionResponse:
    """Move the conversation to another model. It keeps what was said so far."""
    session = _get_session(session_id)
    if data.model:
        try:
            session = await mini.set_model(session_id, data.model)
        except mini.MiniBusyError as exc:
            raise HTTPException(
                status_code=409,
                detail="Still answering the previous message. Wait for it, or interrupt it first.",
            ) from exc
        except ValueError as exc:
            raise HTTPException(status_code=400, detail=str(exc)) from exc
    return _session_response(session)


@router.get("/mini/sessions/{session_id}/models", response_model=models.MiniModelsResponse)
async def list_models(session_id: str) -> models.MiniModelsResponse:
    """The models this conversation can use, across every provider the harness is signed in to."""
    session = _get_session(session_id)
    choices = await mini.list_models(session_id)
    return models.MiniModelsResponse(
        current=session.model,
        models=[models.MiniModelResponse(id=c.id, label=c.label, provider=c.provider) for c in choices],
    )


@router.post("/mini/sessions/{session_id}/messages", response_model=models.MiniMessageResponse, status_code=202)
async def send_message(session_id: str, data: models.MiniMessageRequest) -> models.MiniMessageResponse:
    """Send a message. Read the reply from the events stream, after the returned cursor."""
    _get_session(session_id)
    try:
        cursor = await mini.send_message(session_id, data.text)
    except mini.MiniBusyError as exc:
        raise HTTPException(
            status_code=409,
            detail="Still answering the previous message. Wait for it, or interrupt it first.",
        ) from exc
    return models.MiniMessageResponse(after=cursor)


@router.get("/mini/sessions/{session_id}/events")
async def session_events(session_id: str, after: int = 0):
    """SSE stream of the session's events after cursor ``after``."""
    _get_session(session_id)

    async def event_stream():
        async for seq, event in mini.stream_events(session_id, after=after):
            if event is None:
                # Heartbeat, so a long tool call does not look like a dead connection.
                yield {"event": "ping", "data": "{}"}
                continue
            yield {"event": event["type"], "id": str(seq), "data": json.dumps(event)}

    return EventSourceResponse(event_stream())


@router.post("/mini/sessions/{session_id}/respond", status_code=204)
async def respond(session_id: str, data: models.MiniRespondRequest) -> None:
    """Answer an approval request the harness is waiting on."""
    _get_session(session_id)
    try:
        await mini.respond(session_id, data.request_id, data.answer)
    except ValueError as exc:
        raise HTTPException(status_code=409, detail=str(exc)) from exc


@router.post("/mini/sessions/{session_id}/interrupt", status_code=204)
async def interrupt(session_id: str) -> None:
    """Stop the reply in progress. The session stays open."""
    _get_session(session_id)
    await mini.interrupt(session_id)
