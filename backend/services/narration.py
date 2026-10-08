"""Agent narration lane — ephemeral, streamed TTS for live commentary.

Narration is speech an agent emits *while* it is working, so it should be heard
before the work finishes, it should not clutter history, and the user must be
able to cut it off by talking.

It shares the work engine with the serial generation queue
(``services/task_queue.py``): the profile's engine, on the same backend instance,
so the voice matches normal speech and the model is usually already warm. The
stream waits for the queue to drain before synthesizing (see
``task_queue.wait_until_idle``), which is what keeps it from contending with a
running generation for the device. Audio is streamed chunk by chunk over
``GET /speak/{narration_id}/stream`` instead of being written to the data dir.

Sessions live in memory. A narration nobody listens to produces no audio, no
``generations`` row, and no history noise — and closing the stream cancels it.
"""

import asyncio
import logging
import time
import uuid
from dataclasses import dataclass

from sqlalchemy.orm import Session

from ..utils import timing
from . import settings as settings_service
from .profiles import validate_profile_engine

logger = logging.getLogger(__name__)

# Sessions are ephemeral: created by a speak call, discarded when the stream
# ends. An unclaimed session expires rather than accumulating (a client that
# never connects gets no audio, which is the contract — the pill is the only
# playback surface).
MAX_SESSIONS = 16
SESSION_TTL_SECONDS = 120

# In-process narration shares the work engine, so it waits for the generation
# queue to drain before synthesizing: how long it waits in total, and how often
# it reports progress while waiting.
QUEUE_POLL_SECONDS = 1.0
QUEUE_WAIT_SECONDS = 180.0


@dataclass
class NarrationSession:
    """A pending narration: text resolved, audio not yet synthesized."""

    id: str
    text: str
    profile_id: str
    profile_name: str
    engine: str
    language: str
    reason: str
    created_at: float
    model_size: str | None = None
    claimed_at: float | None = None
    # Set by cancel_all(). A live stream checks this between chunks and stops,
    # which closes the connection to the worker and cancels its synthesis at the
    # next chunk boundary.
    cancelled: bool = False


_sessions: dict[str, NarrationSession] = {}
_engine_locks: dict[str, asyncio.Lock] = {}


def resolve_narration_lane(
    db: Session,
    profile,
    requested_engine: str | None,
) -> tuple[str, str] | None:
    """
    Decide whether this profile can narrate, and with which engine.

    Narration shares the work engine (see the plan's "Open decision"): a separate
    CPU lane was abandoned because LuxTTS's CPU path under-generates short text.
    So the only requirements are that the ``agent_narration`` setting is on and
    that the resolved engine can actually serve this profile's voice type.
    Immediacy is a separate matter — the stream waits for the generation queue to
    be idle before it starts, since it is using the same engine.

    Args:
        db: Database session (for the generation settings row)
        profile: VoiceProfile ORM row to narrate with
        requested_engine: Engine explicitly requested by the caller, if any

    Returns:
        ``(engine, reason)``, or ``None`` if the queued path must be used
    """
    if not settings_service.get_generation_settings(db).agent_narration:
        return None

    engine = (
        requested_engine
        or getattr(profile, "default_engine", None)
        or getattr(profile, "preset_engine", None)
        or "qwen"
    )

    try:
        validate_profile_engine(profile, engine)
    except ValueError:
        return None

    voice_type = getattr(profile, "voice_type", None) or "cloned"
    if voice_type not in ("cloned", "preset"):
        # "designed" profiles have no synthesizable voice yet.
        return None
    return engine, "preset" if voice_type == "preset" else "cloned"


def create_session(
    *,
    text: str,
    profile_id: str,
    profile_name: str,
    engine: str,
    language: str,
    reason: str,
    model_size: str | None = None,
    session_id: str | None = None,
) -> NarrationSession:
    """Register a narration session, sweeping expired entries and evicting if full.

    ``session_id`` lets the caller use its own id — the narration id doubles as
    the voice-loop trace id, and the caller has to mark ``speak_request`` before
    this exists.
    """
    now = time.monotonic()
    _sweep(now)

    while len(_sessions) >= MAX_SESSIONS:
        oldest_id = min(_sessions, key=lambda sid: _sessions[sid].created_at)
        _sessions.pop(oldest_id, None)
        logger.warning("Evicted narration session %s (session cap reached)", oldest_id)

    session = NarrationSession(
        id=session_id or str(uuid.uuid4()),
        text=text,
        profile_id=profile_id,
        profile_name=profile_name,
        engine=engine,
        language=language,
        reason=reason,
        model_size=model_size,
        created_at=now,
    )
    _sessions[session.id] = session
    timing.mark(session.id, "session_created")
    return session


def get_session(narration_id: str) -> NarrationSession | None:
    """Look up a narration session, sweeping expired entries first."""
    _sweep(time.monotonic())
    return _sessions.get(narration_id)


def claim_session(narration_id: str) -> NarrationSession | None:
    """
    Take ownership of a session for streaming.

    Returns ``None`` when the session is missing or has already been claimed,
    so a second listener cannot double-synthesize the same narration.
    """
    session = get_session(narration_id)
    if session is None or session.claimed_at is not None:
        return None
    session.claimed_at = time.monotonic()
    return session


def discard_session(narration_id: str) -> None:
    """Drop a session (stream finished, failed, or was cancelled)."""
    _sessions.pop(narration_id, None)


def cancel_all() -> int:
    """
    Cancel every live narration. Returns how many were cancelled.

    Sessions stay registered so a running stream can still read its own flag and
    finish cleanly (publishing ``speak-end`` with ``status="cancelled"``); the
    flag is what makes the stop immediate rather than "after this chunk".
    """
    live = [sid for sid, session in _sessions.items() if not session.cancelled]
    for sid in live:
        _sessions[sid].cancelled = True
    return len(live)


def engine_stream_lock(engine: str) -> asyncio.Lock:
    """One lock per engine instance — two streams must not share a torch model."""
    lock = _engine_locks.get(engine)
    if lock is None:
        lock = asyncio.Lock()
        _engine_locks[engine] = lock
    return lock


def reset_sessions() -> None:
    """Drop every session and lock (used by tests)."""
    _sessions.clear()
    _engine_locks.clear()


async def run_synthesis(
    queue: "asyncio.Queue",
    *,
    profile_id: str,
    text: str,
    language: str,
    engine: str,
    model_size: str | None,
    wait_for_queue: bool,
    cancel_check: "Callable[[], bool] | None" = None,
) -> None:
    """
    Synthesize a narration sentence by sentence, publishing onto *queue*.

    Items pushed: ``("waiting", waited_ms)``, ``("loading", None)``,
    ``("mark", stage)``, ``("chunk", (index, sample_rate, chunk_text, wav_bytes))``,
    ``("error", exc)``, then a final ``None``. The caller owns the queue and the
    HTTP surface, so the same loop serves the in-process stream and the
    standalone narration worker.

    ``wait_for_queue`` is the difference between the two: in-process narration
    shares the work engine with the generation queue and must wait for it to
    drain, while the worker owns its own engine instance and can synthesize
    immediately.
    """
    from ..backends import (
        engine_has_model_sizes,
        engine_needs_trim,
        engine_retries_runaway,
        get_tts_backend_for_engine,
        load_engine_model,
    )
    from ..database import get_db
    from ..utils.audio import normalize_audio, rms_gain
    from ..utils.chunked_tts import generate_chunked_stream
    from . import profiles, tts

    db = next(get_db())
    try:
        backend = get_tts_backend_for_engine(engine)

        async with engine_stream_lock(engine):
            if wait_for_queue:
                from .task_queue import wait_until_idle

                waited = 0.0
                while not await wait_until_idle(QUEUE_POLL_SECONDS):
                    waited += QUEUE_POLL_SECONDS
                    if waited >= QUEUE_WAIT_SECONDS:
                        raise RuntimeError(
                            "the generation queue is still busy; narration was dropped"
                        )
                    await queue.put(("waiting", int(waited * 1000)))

            await queue.put(("loading", None))

            resolved_size = (model_size or "1.7B") if engine_has_model_sizes(engine) else "default"
            # Sub-stage marks travel on the same queue as the audio, so the
            # caller applies them whether this runs in-process or in the worker
            # process (whose own timing store the main server cannot see).
            await queue.put(("mark", "model_load_start"))
            await load_engine_model(engine, resolved_size)
            await queue.put(("mark", "model_load_done"))

            await queue.put(("mark", "prompt_start"))
            voice_prompt = await profiles.create_voice_prompt_for_profile(
                profile_id,
                db,
                engine=engine,
            )
            await queue.put(("mark", "prompt_done"))

            trim_fn = None
            if engine_needs_trim(engine):
                from ..utils.audio import trim_tts_output

                trim_fn = trim_tts_output
            runaway_detector = None
            if engine_retries_runaway(engine):
                from ..utils.audio import has_tts_runaway

                runaway_detector = has_tts_runaway

            index = 0
            gain: float | None = None
            async for audio, sample_rate, chunk_text in generate_chunked_stream(
                backend,
                text,
                voice_prompt,
                language=language,
                trim_fn=trim_fn,
                runaway_detector=runaway_detector,
            ):
                # The queued path normalizes the whole utterance; narration must
                # do the same or it comes out far quieter than normal speech
                # (~20 dB down for LuxTTS). The gain is computed once from the
                # first chunk and reused, so the level holds across sentences
                # instead of pumping at every boundary.
                if gain is None:
                    gain = rms_gain(audio)
                wav_bytes = tts.audio_to_wav_bytes(
                    normalize_audio(audio, gain=gain), sample_rate
                )
                if cancel_check is not None and cancel_check():
                    # The user silenced agent speech: stop here rather than
                    # synthesizing chunks nobody will hear.
                    logger.info("Narration cancelled mid-synthesis by the user")
                    break
                await queue.put(("chunk", (index, sample_rate, chunk_text, wav_bytes)))
                index += 1
    except Exception as exc:
        await queue.put(("error", exc))
    finally:
        db.close()
        await queue.put(None)


def _sweep(now: float) -> None:
    """Discard sessions that were never claimed within the TTL."""
    expired = [sid for sid, session in _sessions.items() if now - session.created_at > SESSION_TTL_SECONDS]
    for sid in expired:
        _sessions.pop(sid, None)
        logger.info("Expired narration session %s", sid)
