"""Tests for the harness contract: discovery, the registry and the event shapes.

No harness is started here. A fake stands in for an adapter, so these pin what
every adapter must look like to the code above it. No CLI, no provider, no network.
"""

import shutil
from datetime import UTC, datetime

import pytest

from backend import harness
from backend.harness import (
    ApprovalOption,
    ApprovalRequest,
    Harness,
    HarnessError,
    HarnessInfo,
    HarnessSession,
    HarnessSpec,
    LimitReached,
    MessageDone,
    ModelChoice,
    StatusChange,
    TextDelta,
    ToolActivity,
    discovery,
    event_to_dict,
)


class FakeHarness:
    """The smallest thing that satisfies the protocol."""

    async def info(self):
        return HarnessInfo(name="fake", display_name="Fake", installed=True)

    async def start_session(self, cwd, *, resume=None, model=None):
        return HarnessSession(session_id=resume or "s1", cwd=cwd, model=model)

    async def send(self, session_id, text):
        yield StatusChange("running")
        yield TextDelta(text.upper())
        yield MessageDone()

    async def respond(self, session_id, request_id, answer):
        pass

    async def interrupt(self, session_id):
        pass

    async def list_models(self, session_id=None):
        return [ModelChoice(id="provider/model", label="Model")]

    async def set_model(self, session_id, model_id):
        pass

    async def close(self):
        pass


@pytest.fixture
def registry(monkeypatch):
    """A registry holding one fake row, and a count of how often it was built."""
    built: list[FakeHarness] = []

    def factory():
        built.append(FakeHarness())
        return built[-1]

    monkeypatch.setattr(harness, "HARNESSES", {"fake": HarnessSpec("fake", "Fake", factory)})
    harness.reset_harnesses()
    yield built
    harness.reset_harnesses()


@pytest.fixture
def no_path(monkeypatch):
    """Nothing on PATH, as under launchd."""
    monkeypatch.setattr(shutil, "which", lambda name: None)


def _make_cli(directory, name="omp", mode=0o755):
    directory.mkdir(parents=True, exist_ok=True)
    path = directory / name
    path.write_text("#!/bin/sh\n")
    path.chmod(mode)
    return path


# ── Protocol ──────────────────────────────────────────────────────────────


def test_an_adapter_with_every_method_satisfies_the_protocol():
    assert isinstance(FakeHarness(), Harness)


def test_an_object_missing_methods_does_not():
    class NoSend:
        async def info(self):
            pass

    assert not isinstance(NoSend(), Harness)


async def test_send_yields_events_until_the_message_is_done():
    events = [event async for event in FakeHarness().send("s1", "hi")]

    assert events == [StatusChange("running"), TextDelta("HI"), MessageDone()]


# ── Registry ──────────────────────────────────────────────────────────────


def test_a_harness_is_built_once_and_shared(registry):
    first = harness.get_harness("fake")
    second = harness.get_harness("fake")

    assert first is second
    assert len(registry) == 1


def test_reset_drops_the_instance_but_keeps_the_row(registry):
    first = harness.get_harness("fake")
    harness.reset_harnesses()

    assert harness.get_harness("fake") is not first
    assert len(registry) == 2


def test_an_unknown_harness_is_refused_by_name(registry):
    with pytest.raises(ValueError, match=r"Unknown harness: codex.*fake"):
        harness.get_harness("codex")


# ── Events ────────────────────────────────────────────────────────────────


@pytest.mark.parametrize(
    ("event", "expected"),
    [
        (TextDelta("hi"), {"type": "text_delta", "text": "hi", "thinking": False}),
        (MessageDone("cancelled"), {"type": "message_done", "stop_reason": "cancelled"}),
        (StatusChange("blocked", "limit"), {"type": "status_change", "status": "blocked", "reason": "limit"}),
        (
            HarnessError("boom", code=-32603, details="provider said no"),
            {"type": "error", "message": "boom", "code": -32603, "details": "provider said no"},
        ),
        (
            ToolActivity("c1", "ls -la", "completed", kind="execute", raw_input={"cmd": "ls -la"}, raw_output="ok"),
            {
                "type": "tool_activity",
                "call_id": "c1",
                "title": "ls -la",
                "status": "completed",
                "kind": "execute",
                "raw_input": {"cmd": "ls -la"},
                "raw_output": "ok",
            },
        ),
    ],
)
def test_an_event_flattens_to_tagged_data(event, expected):
    assert event_to_dict(event) == expected


def test_an_approval_request_carries_its_options():
    request = ApprovalRequest(
        request_id="r1",
        title="Run rm -rf build",
        options=(ApprovalOption("o1", "Allow once", "allow_once"), ApprovalOption("o2", "Reject", "reject_once")),
        call_id="c1",
    )

    data = event_to_dict(request)

    assert data["type"] == "approval_request"
    assert [option["option_id"] for option in data["options"]] == ["o1", "o2"]
    assert data["options"][0]["kind"] == "allow_once"


def test_a_limit_reports_its_reset_time_as_text():
    resets_at = datetime(2026, 10, 8, 14, 30, tzinfo=UTC)

    data = event_to_dict(LimitReached("Usage limit reached", window="5h", resets_at=resets_at))

    assert data == {
        "type": "limit_reached",
        "message": "Usage limit reached",
        "window": "5h",
        "resets_at": "2026-10-08T14:30:00+00:00",
    }


# ── Discovery ─────────────────────────────────────────────────────────────


def test_a_cli_on_path_is_found(monkeypatch):
    monkeypatch.setattr(shutil, "which", lambda name: f"/somewhere/{name}")

    assert discovery.find_binary("omp") == "/somewhere/omp"


def test_a_cli_off_path_is_found_where_it_installs_itself(monkeypatch, no_path, tmp_path):
    cli = _make_cli(tmp_path / "second")
    monkeypatch.setattr(discovery, "_INSTALL_DIRS", (str(tmp_path / "first"), str(tmp_path / "second")))

    assert discovery.find_binary("omp") == str(cli)


def test_a_file_that_cannot_be_run_is_skipped(monkeypatch, no_path, tmp_path):
    _make_cli(tmp_path / "bin", mode=0o644)
    monkeypatch.setattr(discovery, "_INSTALL_DIRS", (str(tmp_path / "bin"),))

    assert discovery.find_binary("omp") is None


def test_a_path_the_user_set_wins(monkeypatch, tmp_path):
    cli = _make_cli(tmp_path / "custom")
    monkeypatch.setattr(shutil, "which", lambda name: f"/somewhere/{name}")

    assert discovery.find_binary("omp", override=str(cli)) == str(cli)


def test_a_user_path_that_does_not_exist_falls_back_to_the_search(monkeypatch, tmp_path):
    monkeypatch.setattr(shutil, "which", lambda name: f"/somewhere/{name}")

    assert discovery.find_binary("omp", override=str(tmp_path / "missing")) == "/somewhere/omp"


def test_nothing_installed_is_none(monkeypatch, no_path, tmp_path):
    monkeypatch.setattr(discovery, "_INSTALL_DIRS", (str(tmp_path),))

    assert discovery.find_binary("omp") is None
