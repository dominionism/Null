"""Tests for headless playback gating.

Agent speech normally plays through the pill. The backend should only play it
itself when the setting is on, no pill client is subscribed, and the generation
was agent-initiated — a generation the user started in the app is played by the
app.
"""

import asyncio
import uuid
from pathlib import Path

import pytest
from sqlalchemy import create_engine
from sqlalchemy.orm import sessionmaker

from backend import config
from backend.database.models import Base, Generation
from backend.services import generation, playback, settings as settings_service


@pytest.fixture
def db_session(tmp_path, monkeypatch):
    monkeypatch.setattr(config, "_data_dir", tmp_path)
    engine = create_engine("sqlite://")
    Base.metadata.create_all(engine)
    session = sessionmaker(bind=engine)()
    try:
        yield session
    finally:
        session.close()


@pytest.fixture
def scheduled(monkeypatch):
    """Capture the coroutines handed to create_background_task."""
    import backend.services.task_queue as task_queue

    captured: list = []

    def capture(coro):
        captured.append(coro)
        coro.close()  # the play itself is not what these tests assert
        return None

    monkeypatch.setattr(task_queue, "create_background_task", capture)
    return captured


def make_generation(db, audio_path: str, source: str = "mcp") -> str:
    generation_id = str(uuid.uuid4())
    db.add(
        Generation(
            id=generation_id,
            profile_id="p1",
            text="hello",
            language="en",
            audio_path=audio_path,
            source=source,
        )
    )
    db.commit()
    return generation_id


def prepare(tmp_path, *, setting: bool, pill: bool, source: str, monkeypatch, db):
    audio = tmp_path / "generations" / "line.wav"
    audio.parent.mkdir(parents=True, exist_ok=True)
    audio.write_bytes(b"RIFF")
    settings_service.update_capture_settings(db, {"headless_playback": setting})
    monkeypatch.setattr(playback, "pill_attached", lambda: pill)
    generation_id = make_generation(db, "generations/line.wav", source)
    return generation_id, audio


def test_plays_when_headless_and_agent_initiated(tmp_path, db_session, monkeypatch, scheduled):
    generation_id, audio = prepare(
        tmp_path, setting=True, pill=False, source="mcp", monkeypatch=monkeypatch, db=db_session
    )

    generation._maybe_play_headless(generation_id, "generations/line.wav", db_session)

    assert len(scheduled) == 1, "expected the resolved file to be handed to the player"


@pytest.mark.parametrize(
    ("setting", "pill", "source"),
    [
        (False, False, "mcp"),  # switched off
        (True, False, "manual"),  # user-initiated, the app plays it
    ],
)
def test_does_not_play_otherwise(
    tmp_path, db_session, monkeypatch, scheduled, setting, pill, source
):
    generation_id, _ = prepare(
        tmp_path, setting=setting, pill=pill, source=source, monkeypatch=monkeypatch, db=db_session
    )

    generation._maybe_play_headless(generation_id, "generations/line.wav", db_session)

    assert scheduled == []


def test_pill_attached_defers_to_the_ack(tmp_path, db_session, monkeypatch, scheduled):
    """An attached pill gets first refusal on agent speech, but not the last word.

    Nothing plays locally the moment the generation lands: the backend waits for
    the pill to take the audio, because a pill that dropped its cycle looks
    exactly like a pill that is playing.
    """
    generation_id, _ = prepare(
        tmp_path, setting=True, pill=True, source="mcp", monkeypatch=monkeypatch, db=db_session
    )

    generation._maybe_play_headless(generation_id, "generations/line.wav", db_session)

    assert len(scheduled) == 1, "expected the deferred ack check to be scheduled"


@pytest.fixture
def pill_ack(monkeypatch):
    """Record what the deferred check does, with the grace period collapsed."""
    from backend.mcp_server import events as mcp_events

    played: list = []
    published: list = []

    async def fake_play(path):
        played.append(path)

    monkeypatch.setattr(playback, "play_file", fake_play)
    monkeypatch.setattr(mcp_events, "publish", lambda kind, payload: published.append((kind, payload)))
    monkeypatch.setattr(generation, "PILL_TAKEOVER_GRACE_SECONDS", 0)
    playback._audio_served.clear()
    return played, published


async def test_plays_when_the_pill_never_takes_the_audio(
    tmp_path, db_session, monkeypatch, pill_ack
):
    """A pill that drops its cycle must not turn into silence."""
    played, published = pill_ack
    generation_id, audio = prepare(
        tmp_path, setting=True, pill=True, source="mcp", monkeypatch=monkeypatch, db=db_session
    )

    await generation._play_headless_if_the_pill_never_takes_it(generation_id, audio)

    assert played == [audio], "the user must hear it even when the pill drops the cycle"
    # The pill's own cycle is stopped, in case it was about to play it too.
    assert published and published[0][0] == "speak-end"
    assert published[0][1]["status"] == "cancelled"


async def test_stays_quiet_when_the_pill_takes_the_audio(
    tmp_path, db_session, monkeypatch, pill_ack
):
    """A pill that took the audio owns the playback — no double audio."""
    played, published = pill_ack
    generation_id, audio = prepare(
        tmp_path, setting=True, pill=True, source="mcp", monkeypatch=monkeypatch, db=db_session
    )
    playback.mark_audio_served(generation_id)

    await generation._play_headless_if_the_pill_never_takes_it(generation_id, audio)

    assert played == []
    assert published == []


def test_missing_audio_path_is_ignored(db_session, monkeypatch, scheduled):
    monkeypatch.setattr(playback, "pill_attached", lambda: False)
    settings_service.update_capture_settings(db_session, {"headless_playback": True})
    generation_id = make_generation(db_session, "")

    generation._maybe_play_headless(generation_id, "", db_session)

    assert scheduled == []


async def test_play_file_uses_the_platform_player(tmp_path, monkeypatch):
    """The player command is platform-specific; on macOS it is afplay."""
    calls: list[list[str]] = []

    class FakeProcess:
        async def wait(self) -> int:
            return 0

    async def fake_exec(*args, **kwargs):
        calls.append(list(args))
        return FakeProcess()

    monkeypatch.setattr(asyncio, "create_subprocess_exec", fake_exec)
    audio = tmp_path / "line.wav"
    audio.write_bytes(b"RIFF")

    await playback.play_file(audio)

    assert len(calls) == 1
    assert calls[0][0] == playback._player_command()[0]
    assert calls[0][-1] == str(audio)
