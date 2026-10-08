"""Tests for the agent-voice gate and the hard stop.

Two behaviours the user asked for by name: turning voice off must mean agents
cannot speak *at all* (not merely that a player is silenced), and the stop chord
must reach narration, headless playback, and the pill at once.
"""

import pytest
from fastapi import HTTPException
from sqlalchemy import create_engine
from sqlalchemy.orm import sessionmaker

from backend import models
from backend.database import Base, VoiceProfile
from backend.routes import speak as speak_routes
from backend.services import narration, playback, settings as settings_service


@pytest.fixture
def db_session():
    engine = create_engine("sqlite://")
    Base.metadata.create_all(engine)
    session = sessionmaker(bind=engine)()
    session.add(VoiceProfile(id="p1", name="Test Profile"))
    session.commit()
    try:
        yield session
    finally:
        session.close()


def _mute(db, muted: bool) -> None:
    settings_service.update_capture_settings(db, {"agent_voice_enabled": not muted})


async def test_speak_route_refuses_when_agent_voice_is_off(db_session):
    """A muted agent must be told, not silently ignored."""
    _mute(db_session, True)

    with pytest.raises(HTTPException) as exc:
        await speak_routes.speak(
            data=models.SpeakRequest(text="hello", profile_id="p1"),
            request=None,  # the gate raises before the request is touched
            db=db_session,
        )

    assert exc.value.status_code == 409
    # The message is for the model as much as the user: it must say "do not retry".
    assert "Do not retry" in exc.value.detail


async def test_narrate_route_refuses_when_agent_voice_is_off(db_session):
    """Narration is the lane an agent chatters on, so it is gated too."""
    _mute(db_session, True)

    with pytest.raises(HTTPException) as exc:
        await speak_routes.narrate(
            data=models.SpeakRequest(text="working on it", profile_id="p1"),
            request=None,
            db=db_session,
        )

    assert exc.value.status_code == 409


def test_the_mute_flag_round_trips(db_session):
    """The UI toggle persists, and the gate reads what was written."""
    _mute(db_session, True)
    assert settings_service.agent_voice_enabled(db_session) is False

    _mute(db_session, False)
    assert settings_service.agent_voice_enabled(db_session) is True


def test_cancel_all_marks_every_live_session():
    """A live stream reads this flag between chunks and stops."""
    narration.reset_sessions()
    first = narration.create_session(
        profile_id="p1", profile_name="Test", text="one", engine="luxtts", language="en", reason="test"
    )
    second = narration.create_session(
        profile_id="p1", profile_name="Test", text="two", engine="luxtts", language="en", reason="test"
    )

    cancelled = narration.cancel_all()

    assert cancelled == 2
    first_session = narration.get_session(first.id)
    second_session = narration.get_session(second.id)
    assert first_session is not None and first_session.cancelled is True
    assert second_session is not None and second_session.cancelled is True
    # Idempotent: a second stop has nothing left to cancel.
    assert narration.cancel_all() == 0
    narration.reset_sessions()


async def test_stop_playback_terminates_the_player(monkeypatch):
    """Headless playback is a subprocess, so silencing it means killing it."""
    terminated = []

    class FakeProcess:
        returncode = None

        def terminate(self):
            terminated.append(True)
            self.returncode = -15

    monkeypatch.setattr(playback, "_current_player", FakeProcess())

    assert playback.stop_playback() is True
    assert terminated == [True]
    # Once terminated it is no longer the in-flight player.
    assert playback.stop_playback() is False


async def test_stop_playback_is_a_noop_without_a_player(monkeypatch):
    monkeypatch.setattr(playback, "_current_player", None)
    assert playback.stop_playback() is False
