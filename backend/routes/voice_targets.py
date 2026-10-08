"""Live agents, and delivery of a spoken transcript into one of them.

Two surfaces:

- ``GET /agents`` — what herdr can see right now (pane, kind, lifecycle state).
- ``POST /voice-targets/message`` — hand a transcript to the configured agent.

The target itself is a capture setting (``agent_target``), so choosing where
your voice goes reuses the existing settings endpoint rather than adding
another one. Null means "paste into the focused field", which is the behaviour
that predates this feature.

Deliberately *not* gated by ``agent_voice_enabled``: that flag silences what
agents say, and talking to them is input, not output. Muting your agents must
never mute you.
"""

from __future__ import annotations

import logging

from fastapi import APIRouter, Depends, HTTPException
from sqlalchemy.orm import Session

from .. import models
from ..database import get_db
from ..services import voice_targets
from ..services.settings import get_capture_settings
from ..utils import timing

logger = logging.getLogger(__name__)

router = APIRouter(tags=["voice-targets"])


@router.get("/agents", response_model=models.AgentListResponse)
async def list_agents() -> models.AgentListResponse:
    """Agents herdr can see, for the settings picker and the pill.

    A missing herdr is a normal state, not an error: the response says so and
    the caller falls back to pasting.
    """
    if not voice_targets.available():
        return models.AgentListResponse(available=False, agents=[])

    try:
        agents = voice_targets.list_agents()
    except voice_targets.VoiceTargetError as exc:
        logger.warning("Could not list agents: %s", exc)
        return models.AgentListResponse(available=False, agents=[], error=str(exc))

    return models.AgentListResponse(
        available=True,
        agents=[
            models.AgentTargetResponse(
                target=a.target,
                agent=a.agent,
                status=a.status,
                ready=a.ready,
                cwd=a.cwd,
                title=a.title,
                focused=a.focused,
            )
            for a in agents
        ],
    )


@router.post("/voice-targets/message", response_model=models.VoiceDeliveryResponse)
async def deliver_message(
    data: models.VoiceMessageRequest,
    db: Session = Depends(get_db),
) -> models.VoiceDeliveryResponse:
    """Send a transcript to the configured agent as a voice turn.

    The agent is re-listed before every send, so a target that has exited
    reports itself instead of swallowing the words.
    """
    settings = get_capture_settings(db)
    target = (data.target or settings.agent_target or "").strip()
    if not target:
        raise HTTPException(
            status_code=409,
            detail=(
                "No agent target is set. Pick one in Settings → Captures → "
                "Talk to agent, or bind a chord and choose it there."
            ),
        )

    if not voice_targets.available():
        raise HTTPException(
            status_code=503,
            detail="herdr is not installed, so Voicebox cannot reach an agent session directly.",
        )

    try:
        live = {a.target: a for a in voice_targets.list_agents()}
    except voice_targets.VoiceTargetError as exc:
        raise HTTPException(status_code=503, detail=str(exc)) from exc

    agent = live.get(target)
    if agent is None:
        raise HTTPException(
            status_code=409,
            detail=(
                f"The agent at {target} is no longer running. Pick a live one in "
                "Settings → Captures → Talk to agent."
            ),
        )

    timing.mark(data.turn_id, "deliver_start")
    try:
        voice_targets.send_prompt(target, data.text, voice_turn=data.voice_turn)
    except voice_targets.VoiceTargetError as exc:
        if exc.blocked:
            raise HTTPException(
                status_code=409,
                detail=(
                    f"{agent.agent} is waiting on you (an approval or a question), so the "
                    "message was not delivered. Answer it in the session, then talk again."
                ),
            ) from exc
        raise HTTPException(status_code=502, detail=str(exc)) from exc
    finally:
        # The capture half of the turn ends here — the agent's reply is its own.
        timing.mark(data.turn_id, "deliver_done")
        timing.finish_turn(data.turn_id)

    logger.info(
        "Voice message delivered to %s (%s, %s)", target, agent.agent, agent.status
    )
    return models.VoiceDeliveryResponse(
        delivered=True, target=target, agent=agent.agent, status=agent.status
    )
