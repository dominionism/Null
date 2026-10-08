"""Tests for the startup model warm-up's gating rules.

The warm-up exists to move model loads off the user's first turn, and its rules
are all about what it must *not* do: never run when the user turned it off, never
run for a profile that cannot narrate, and never turn a startup into a model
download. These tests pin those rules without loading a real model.
"""

from types import SimpleNamespace

import pytest
from sqlalchemy import create_engine
from sqlalchemy.orm import sessionmaker

from backend.database.models import Base, CaptureSettings, VoiceProfile
from backend.routes.narration_worker import warm
from backend.services import narration


@pytest.fixture
def db_session():
    engine = create_engine("sqlite://")
    Base.metadata.create_all(engine)
    session = sessionmaker(bind=engine)()
    try:
        yield session
    finally:
        session.close()


@pytest.fixture(autouse=True)
def _clean_state():
    narration.reset_sessions()
    yield
    narration.reset_sessions()


@pytest.fixture
def _use_db(db_session, monkeypatch):
    monkeypatch.setattr("backend.database.get_db", lambda: iter([db_session]))


@pytest.fixture
def _no_model_load(monkeypatch):
    """Fail loudly if the warm-up tries to load (or download) a model."""
    monkeypatch.setattr(
        "backend.backends.get_tts_backend_for_engine",
        lambda engine: SimpleNamespace(_is_model_cached=lambda *a: False),
    )

    async def _boom(*args, **kwargs):
        raise AssertionError("warm-up must not load an uncached model")

    monkeypatch.setattr("backend.backends.load_engine_model", _boom)


def _cloned_profile(engine: str = "luxtts") -> VoiceProfile:
    return VoiceProfile(id="p1", name="Me", voice_type="cloned", default_engine=engine)


def _set_default_voice(db_session, voice_id: str | None) -> None:
    db_session.add(CaptureSettings(id=1, default_playback_voice_id=voice_id))
    db_session.commit()


@pytest.mark.asyncio
async def test_no_default_voice_is_a_noop(_use_db, _no_model_load):
    result = await warm()

    assert result["warmed"] is False
    assert result["reason"] == "no playback voice configured"


@pytest.mark.asyncio
async def test_missing_profile_is_a_noop(_use_db, _no_model_load, db_session):
    _set_default_voice(db_session, "gone")

    result = await warm()

    assert result["warmed"] is False
    assert result["reason"] == "playback voice no longer exists"


@pytest.mark.asyncio
async def test_narration_off_is_a_noop(_use_db, _no_model_load, db_session):
    db_session.add(_cloned_profile())
    _set_default_voice(db_session, "p1")
    from backend.database.models import GenerationSettings

    db_session.add(GenerationSettings(id=1, agent_narration=False))
    db_session.commit()

    result = await warm()

    assert result["warmed"] is False
    assert result["reason"] == "narration lane unavailable"


@pytest.mark.asyncio
async def test_uncached_model_is_never_downloaded(_use_db, _no_model_load, db_session):
    """A startup warm-up must not pull a model the user never asked to download."""
    db_session.add(_cloned_profile())
    _set_default_voice(db_session, "p1")
    db_session.commit()

    result = await warm()

    assert result["warmed"] is False
    assert result["reason"] == "model not cached"


@pytest.mark.asyncio
async def test_warm_models_on_startup_off_is_a_noop(_use_db, _no_model_load, db_session):
    db_session.add(CaptureSettings(id=1, default_playback_voice_id=None, warm_models_on_startup=False))
    db_session.commit()

    result = await warm()

    assert result["warmed"] is False
    assert result["reason"] == "no playback voice configured"


@pytest.mark.asyncio
async def test_cached_model_is_loaded_with_the_lane_engine(_use_db, monkeypatch, db_session):
    """The warm-up loads exactly what the real narration path would ask for."""
    loaded: list[tuple] = []

    monkeypatch.setattr(
        "backend.backends.get_tts_backend_for_engine",
        lambda engine: SimpleNamespace(_is_model_cached=lambda *a: True),
    )

    async def _record(engine, size):
        loaded.append((engine, size))

    monkeypatch.setattr("backend.backends.load_engine_model", _record)

    db_session.add(_cloned_profile("luxtts"))
    _set_default_voice(db_session, "p1")
    db_session.commit()

    result = await warm()

    assert result == {"warmed": True, "engine": "luxtts", "model_size": "default"}
    assert loaded == [("luxtts", "default")]
