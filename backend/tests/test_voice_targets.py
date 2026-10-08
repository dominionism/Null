"""Tests for the voice-target transport.

The transport shells out to herdr, so these pin the *contract* with a fake
binary: the JSON shapes it must accept, the marker it must apply, and the
errors it must surface rather than swallow. No herdr, no agents, no network.
"""

import json
import subprocess
from types import SimpleNamespace

import pytest

from backend.services import voice_targets
from backend.services.voice_targets import VOICE_TURN_MARKER, AgentTarget, VoiceTargetError


@pytest.fixture
def fake_herdr(monkeypatch):
    """Capture argv and return canned output, as if herdr ran."""
    calls: list[list[str]] = []
    state = {"stdout": "", "stderr": "", "returncode": 0}

    def fake_run(argv, **kwargs):
        calls.append(list(argv))
        return SimpleNamespace(
            stdout=state["stdout"], stderr=state["stderr"], returncode=state["returncode"]
        )

    monkeypatch.setattr(voice_targets, "herdr_binary", lambda: "/fake/herdr")
    monkeypatch.setattr(subprocess, "run", fake_run)
    return SimpleNamespace(calls=calls, state=state)


def _envelope(result=None, error=None):
    payload = {"id": "cli:test"}
    if error is not None:
        payload["error"] = error
    else:
        payload["result"] = result or {}
    return json.dumps(payload)


def test_lists_agents_with_state_and_identity(fake_herdr):
    fake_herdr.state["stdout"] = _envelope(
        {
            "agents": [
                {
                    "agent": "omp",
                    "agent_status": "working",
                    "pane_id": "wM:p1",
                    "cwd": "/tmp/project",
                    "terminal_title_stripped": "Clone voice",
                    "focused": True,
                },
                {
                    "agent": "claude",
                    "agent_status": "idle",
                    "pane_id": "wY:p1",
                    "terminal_title_stripped": "Resume",
                },
            ]
        }
    )

    agents = voice_targets.list_agents()

    assert [a.target for a in agents] == ["wM:p1", "wY:p1"]
    assert agents[0].agent == "omp"
    assert agents[0].status == "working"
    assert agents[0].ready is False
    assert agents[1].status == "idle"
    assert agents[1].ready is True  # idle and done both mean ready
    assert agents[0].focused is True
    assert fake_herdr.calls[0][1:] == ["agent", "list"]


def test_unknown_status_is_not_trusted(fake_herdr):
    """An unrecognised state must not read as ready."""
    fake_herdr.state["stdout"] = _envelope(
        {"agents": [{"agent": "omp", "agent_status": "wat", "pane_id": "w1:p1"}]}
    )

    agents = voice_targets.list_agents()

    assert agents[0].status == "unknown"
    assert agents[0].ready is False


def test_rows_without_a_pane_id_are_skipped(fake_herdr):
    fake_herdr.state["stdout"] = _envelope({"agents": [{"agent": "omp", "agent_status": "idle"}]})
    assert voice_targets.list_agents() == []


def test_error_object_becomes_a_typed_failure(fake_herdr):
    """herdr's error code is preserved, and `blocked` is recognised."""
    fake_herdr.state["stdout"] = _envelope(
        error={"code": "agent_blocked", "message": "agent is blocked"}
    )

    with pytest.raises(VoiceTargetError) as exc:
        voice_targets.send_prompt("w1:p1", "hello")

    assert exc.value.code == "agent_blocked"
    assert exc.value.blocked is True
    assert "blocked" in str(exc.value)


def test_nonzero_exit_surfaces_the_reason(fake_herdr):
    fake_herdr.state["returncode"] = 1
    fake_herdr.state["stderr"] = "herdr: no such session\nsecond line\n"

    with pytest.raises(VoiceTargetError) as exc:
        voice_targets.list_agents()

    assert "no such session" in str(exc.value)
    assert exc.value.code == "herdr_failed"


def test_missing_herdr_is_reported_not_crashed(monkeypatch):
    monkeypatch.setattr(voice_targets, "herdr_binary", lambda: None)
    assert voice_targets.available() is False

    with pytest.raises(VoiceTargetError) as exc:
        voice_targets.list_agents()

    assert exc.value.code == "herdr_missing"
    assert "paste" in str(exc.value)  # tells the caller what happens instead


def test_a_voice_turn_is_prefixed_with_the_marker(fake_herdr):
    """The marker is how the agent knows to answer aloud."""
    fake_herdr.state["stdout"] = _envelope({})

    voice_targets.send_prompt("wY:p1", "what are you working on")

    argv = fake_herdr.calls[0]
    assert argv[1:3] == ["agent", "prompt"]
    assert argv[3] == "wY:p1"
    assert argv[4].startswith(VOICE_TURN_MARKER)
    assert argv[4].endswith("what are you working on")


def test_a_plain_send_has_no_marker(fake_herdr):
    fake_herdr.state["stdout"] = _envelope({})

    voice_targets.send_prompt("wY:p1", "plain text", voice_turn=False)

    assert fake_herdr.calls[0][4] == "plain text"


def test_empty_text_and_missing_target_are_refused_before_shelling_out(fake_herdr):
    with pytest.raises(VoiceTargetError):
        voice_targets.send_prompt("", "hello")
    with pytest.raises(VoiceTargetError):
        voice_targets.send_prompt("wY:p1", "   ")

    assert fake_herdr.calls == []  # nothing was executed


def test_agent_target_ready_means_ready_for_input():
    assert AgentTarget("w1:p1", "omp", "idle").ready is True
    assert AgentTarget("w1:p1", "omp", "done").ready is True
    assert AgentTarget("w1:p1", "omp", "blocked").ready is False
    assert AgentTarget("w1:p1", "omp", "working").ready is False
