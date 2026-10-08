"""Tests for the voice-loop turn trace.

One record per turn, joined across the chord and the first audible audio, so the
loop's stages can be ranked by wall-clock cost instead of guessed
(``Context/Plans/VoiceLoopLatency.md``). The marks are dev instrumentation, so
what matters is that they never lie about ordering, never double-record, and
never affect the loop when they are switched off.
"""

import json

import pytest

from backend import config
from backend.utils import timing


@pytest.fixture(autouse=True)
def _clean_traces():
    """Module-level ring buffer — no test may inherit another's turns."""
    timing.clear()
    yield
    timing.clear()


def _trace_file(tmp_path, monkeypatch):
    """Point the trace store at a tmp data dir; returns the JSONL path."""
    monkeypatch.setattr(config, "get_data_dir", lambda: tmp_path)
    return tmp_path / "logs" / "turns.jsonl"


def test_marks_join_one_turn_in_order():
    timing.begin_turn("turn-1", anchor_epoch_ms=1_700_000_000_000)
    assert timing.mark("turn-1", "upload_start") is True
    assert timing.mark("turn-1", "stt_start") is True
    assert timing.mark("turn-1", "stt_done", transcript_chars=42) is True

    (trace,) = timing.latest()
    assert trace.turn_id == "turn-1"
    assert trace.anchor_epoch_ms == 1_700_000_000_000
    assert [stage for stage, _at, _meta in trace.marks] == [
        "upload_start",
        "stt_start",
        "stt_done",
    ]
    # Monotonic and non-decreasing: a stage cannot be recorded before its turn.
    times = [at for _stage, at, _meta in trace.marks]
    assert times == sorted(times)
    assert trace.marks[-1][2] == {"transcript_chars": 42}


def test_a_stage_is_recorded_only_once():
    """Both sides of a boundary mark the same stage; the first write wins."""
    timing.begin_turn("turn-1")
    assert timing.mark("turn-1", "first_chunk") is True
    assert timing.mark("turn-1", "first_chunk") is False

    (trace,) = timing.latest()
    assert [stage for stage, _at, _meta in trace.marks] == ["first_chunk"]


def test_a_mark_before_its_begin_is_not_lost():
    """The backend can mark before the client's begin lands."""
    assert timing.mark("turn-2", "stt_start") is True

    timing.begin_turn("turn-2", anchor_epoch_ms=123)
    (trace,) = timing.latest()
    assert trace.turn_id == "turn-2"
    assert trace.anchor_epoch_ms == 123
    assert [stage for stage, _at, _meta in trace.marks] == ["stt_start"]


def test_begin_is_idempotent_and_keeps_the_first_anchor():
    first = timing.begin_turn("turn-3", anchor_epoch_ms=100)
    second = timing.begin_turn("turn-3", anchor_epoch_ms=999)

    assert first is second
    assert second.anchor_epoch_ms == 100


def test_turn_end_finishes_once_and_writes_one_line(tmp_path, monkeypatch):
    trace_file = _trace_file(tmp_path, monkeypatch)
    timing.begin_turn("turn-4")
    timing.mark("turn-4", "upload_start")

    timing.mark("turn-4", timing.END_STAGE)
    assert timing.latest()[0].finished is True

    # A second close (the client's finish racing the server's) must not append.
    timing.finish_turn("turn-4")
    timing.mark("turn-4", timing.END_STAGE)

    lines = trace_file.read_text().splitlines()
    assert len(lines) == 1
    written = json.loads(lines[0])
    assert written["turn_id"] == "turn-4"
    assert written["finished"] is True
    assert [mark["stage"] for mark in written["marks"]] == ["upload_start", "turn_end"]


def test_finishing_a_turn_nobody_started_is_harmless():
    assert timing.finish_turn("never-seen") is None


def test_tracing_off_records_nothing(monkeypatch, tmp_path):
    trace_file = _trace_file(tmp_path, monkeypatch)
    monkeypatch.setenv(timing.ENV_VAR, "0")

    timing.begin_turn("turn-5")
    assert timing.mark("turn-5", "stt_start") is False
    assert timing.finish_turn("turn-5") is None
    assert timing.latest() == []
    assert not trace_file.exists()


def test_latest_is_newest_first_and_the_buffer_is_bounded(monkeypatch):
    monkeypatch.setattr(timing, "MAX_TRACES", 3)
    for index in range(5):
        timing.begin_turn(f"turn-{index}")

    ids = [trace.turn_id for trace in timing.latest()]
    assert ids == ["turn-4", "turn-3", "turn-2"]

    assert timing.clear() == 3
    assert timing.latest() == []
