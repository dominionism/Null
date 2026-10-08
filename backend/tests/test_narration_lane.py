"""Tests for the agent narration lane: engine availability and session registry.

Narration shares the work engine with the serial generation queue (a separate CPU
lane was abandoned — LuxTTS's CPU path under-generates short text), so the lane's
own rules are: the setting is on, and the resolved engine can serve the profile's
voice type. These tests pin that rule and the in-memory session lifecycle.
"""

import time
from types import SimpleNamespace

import pytest
from sqlalchemy import create_engine
from sqlalchemy.orm import sessionmaker

from backend.database.models import Base
from backend.services import narration, settings as settings_service


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


def kokoro_preset() -> SimpleNamespace:
    return SimpleNamespace(
        id="p-kokoro",
        voice_type="preset",
        preset_engine="kokoro",
        preset_voice_id="am_adam",
        design_prompt=None,
        personality=None,
        default_engine=None,
    )


def qwen_preset() -> SimpleNamespace:
    return SimpleNamespace(
        id="p-qwen",
        voice_type="preset",
        preset_engine="qwen_custom_voice",
        preset_voice_id="Ethan",
        design_prompt=None,
        personality=None,
        default_engine=None,
    )


def cloned(default_engine: str | None = None) -> SimpleNamespace:
    return SimpleNamespace(
        id="p-cloned",
        voice_type="cloned",
        preset_engine=None,
        preset_voice_id=None,
        design_prompt=None,
        personality=None,
        default_engine=default_engine,
    )


@pytest.mark.parametrize(
    ("profile_factory", "requested_engine", "expected"),
    [
        # A preset narrates with its own preset engine.
        (kokoro_preset, None, ("kokoro", "preset")),
        (qwen_preset, None, ("qwen_custom_voice", "preset")),
        # A cloned profile narrates with its default engine, else the fallback.
        (lambda: cloned("luxtts"), None, ("luxtts", "cloned")),
        (lambda: cloned("qwen"), None, ("qwen", "cloned")),
        (cloned, None, ("qwen", "cloned")),
        # An explicit engine wins, as long as it can serve the profile.
        (lambda: cloned("luxtts"), "chatterbox", ("chatterbox", "cloned")),
        # A preset can only use its own engine — mismatches fall back to the queue.
        (kokoro_preset, "qwen", None),
        # A cloned profile cannot use a preset-only engine.
        (cloned, "kokoro", None),
    ],
)
def test_resolve_narration_lane_truth_table(db_session, profile_factory, requested_engine, expected):
    profile = profile_factory()
    assert narration.resolve_narration_lane(db_session, profile, requested_engine) == expected


def test_resolve_narration_lane_honours_kill_switch(db_session):
    settings_service.update_generation_settings(db_session, {"agent_narration": False})
    assert narration.resolve_narration_lane(db_session, kokoro_preset(), None) is None
    assert narration.resolve_narration_lane(db_session, cloned("luxtts"), None) is None


def test_resolve_narration_lane_rejects_designed_profile(db_session):
    designed = SimpleNamespace(
        id="p-designed",
        voice_type="designed",
        preset_engine=None,
        preset_voice_id=None,
        design_prompt="an old radio host",
        personality=None,
        default_engine=None,
    )
    assert narration.resolve_narration_lane(db_session, designed, None) is None


def test_session_lifecycle():
    session = narration.create_session(
        text="Reading the auth middleware.",
        profile_id="p1",
        profile_name="Morgan",
        engine="luxtts",
        language="en",
        reason="cloned",
    )

    assert narration.get_session(session.id) is session
    assert session.claimed_at is None

    claimed = narration.claim_session(session.id)
    assert claimed is session
    # A second listener must not be able to double-synthesize it.
    assert narration.claim_session(session.id) is None

    narration.discard_session(session.id)
    assert narration.get_session(session.id) is None


def test_unknown_session_is_none():
    assert narration.get_session("does-not-exist") is None


def test_sessions_expire_after_ttl():
    session = narration.create_session(
        text="Ticking.",
        profile_id="p1",
        profile_name="Morgan",
        engine="luxtts",
        language="en",
        reason="cloned",
    )
    session.created_at = time.monotonic() - narration.SESSION_TTL_SECONDS - 1

    assert narration.get_session(session.id) is None


def test_session_cap_evicts_the_oldest():
    sessions = [
        narration.create_session(
            text=f"Line {i}.",
            profile_id="p1",
            profile_name="Morgan",
            engine="luxtts",
            language="en",
            reason="cloned",
        )
        for i in range(narration.MAX_SESSIONS)
    ]
    oldest = sessions[0]

    newest = narration.create_session(
        text="The newest line.",
        profile_id="p1",
        profile_name="Morgan",
        engine="luxtts",
        language="en",
        reason="cloned",
    )

    assert narration.get_session(oldest.id) is None
    assert narration.get_session(newest.id) is newest


def test_engine_stream_lock_is_per_engine_and_stable():
    assert narration.engine_stream_lock("kokoro") is narration.engine_stream_lock("kokoro")
    assert narration.engine_stream_lock("kokoro") is not narration.engine_stream_lock("luxtts")
