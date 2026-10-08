"""Tests for the mini: the session service, its routes and the lock on them.

A fake harness stands in for the agent, so these pin what the mini does with
whatever a harness reports: buffer it, hand it to a reader that arrives late,
and refuse anyone who should not be talking to an agent on this machine.
"""

import asyncio
import json

import pytest
from fastapi import FastAPI
from starlette.testclient import TestClient

from backend import config, harness
from backend.harness import (
    ApprovalOption,
    ApprovalRequest,
    HarnessInfo,
    HarnessSession,
    HarnessSpec,
    MessageDone,
    ModelChoice,
    StatusChange,
    TextDelta,
)
from backend.routes.mini import router
from backend.services import mini, mini_auth


class FakeHarness:
    """Answers every message with its text in capitals, after an optional approval."""

    def __init__(self):
        self.ask_first = False
        self.fail = False
        self.interrupted = []
        self.answers = []
        self.model_calls = []
        self._release = asyncio.Event()

    async def info(self):
        return HarnessInfo(name="fake", display_name="Fake", installed=True, binary="/fake", version="1")

    async def start_session(self, cwd, *, resume=None, model=None):
        return HarnessSession(session_id="s1", cwd=cwd, model="provider/model")

    async def send(self, session_id, text):
        yield StatusChange("running")
        if self.fail:
            raise RuntimeError("the agent fell over")
        if self.ask_first:
            yield StatusChange("needs_input")
            yield ApprovalRequest("r1", "Run it?", (ApprovalOption("yes", "Allow", "allow_once"),))
            await self._release.wait()
            yield StatusChange("running")
        yield TextDelta(text.upper())
        yield StatusChange("ready")
        yield MessageDone()

    async def respond(self, session_id, request_id, answer):
        if request_id != "r1":
            raise ValueError(f"No approval request {request_id}")
        self.answers.append(answer)
        self._release.set()

    async def interrupt(self, session_id):
        self.interrupted.append(session_id)
        self._release.set()

    async def list_models(self, session_id=None):
        return [
            ModelChoice(id="provider/model", label="Model", provider="provider"),
            ModelChoice(id="other/fast", label="Fast", provider="other"),
        ]

    async def set_model(self, session_id, model_id):
        if model_id not in {choice.id for choice in await self.list_models()}:
            raise ValueError(f"Fake has no model named {model_id}")
        self.model_calls.append(model_id)

    async def close(self):
        pass


@pytest.fixture
def fake(monkeypatch, tmp_path):
    """The mini wired to a fake harness and an empty data directory."""
    instance = FakeHarness()
    monkeypatch.setattr(harness, "HARNESSES", {"fake": HarnessSpec("fake", "Fake", lambda: instance)})
    monkeypatch.setattr(mini, "DEFAULT_HARNESS", "fake")
    monkeypatch.setattr(mini, "_sessions", {})
    monkeypatch.setattr(config, "_data_dir", tmp_path)
    monkeypatch.setattr(mini_auth, "_cached", None)
    harness.reset_harnesses()
    yield instance
    harness.reset_harnesses()


async def _read_all(session_id, after=0):
    return [event async for _, event in mini.stream_events(session_id, after=after)]


def _types(events):
    return [event["type"] for event in events]


# ── Service ───────────────────────────────────────────────────────────────


async def test_a_session_opens_in_the_default_workspace(fake, tmp_path):
    session = await mini.create_session()

    assert session.cwd == str(tmp_path / "mini" / "workspace")
    assert (tmp_path / "mini" / "workspace").is_dir()
    assert session.model == "provider/model"
    assert session.status == "ready"


async def test_a_directory_that_does_not_exist_is_refused(fake, tmp_path):
    with pytest.raises(ValueError, match="Not a directory"):
        await mini.create_session(cwd=str(tmp_path / "nowhere"))


async def test_a_message_is_answered_and_every_event_is_kept_in_order(fake):
    session = await mini.create_session()

    after = await mini.send_message(session.id, "hi")
    events = await _read_all(session.id, after)

    assert _types(events) == ["user_message", "status_change", "text_delta", "status_change", "message_done"]
    assert events[0]["text"] == "hi"
    assert events[2]["text"] == "HI"
    assert session.status == "ready"
    assert not session.busy


async def test_a_reader_that_arrives_late_gets_the_whole_reply(fake):
    session = await mini.create_session()
    await mini.send_message(session.id, "hi")
    await session.task

    assert "text_delta" in _types(await _read_all(session.id))


async def test_a_reader_resumes_from_its_cursor(fake):
    session = await mini.create_session()
    await mini.send_message(session.id, "one")
    await session.task
    after = await mini.send_message(session.id, "two")

    events = await _read_all(session.id, after)

    assert [event["text"] for event in events if event["type"] in ("user_message", "text_delta")] == ["two", "TWO"]


async def test_a_second_message_is_refused_while_the_first_is_being_answered(fake):
    fake.ask_first = True
    session = await mini.create_session()
    await mini.send_message(session.id, "hi")

    with pytest.raises(mini.MiniBusyError):
        await mini.send_message(session.id, "again")

    await mini.interrupt(session.id)
    await session.task


async def test_an_approval_holds_the_reply_until_it_is_answered(fake):
    fake.ask_first = True
    session = await mini.create_session()
    after = await mini.send_message(session.id, "hi")

    seen = []
    async for _, event in mini.stream_events(session.id, after=after):
        seen.append(event["type"])
        if event["type"] == "approval_request":
            assert session.status == "needs_input"
            await mini.respond(session.id, event["request_id"], event["options"][0]["option_id"])

    assert fake.answers == ["yes"]
    assert seen[-1] == "message_done"
    assert session.status == "ready"


async def test_a_quiet_reply_sends_heartbeats(fake):
    fake.ask_first = True
    session = await mini.create_session()
    after = await mini.send_message(session.id, "hi")

    beats = 0
    async for _, event in mini.stream_events(session.id, after=after, heartbeat=0.01):
        if event is None:
            beats += 1
            if beats == 2:
                await mini.interrupt(session.id)

    assert beats >= 2
    assert fake.interrupted == [session.id]


async def test_a_harness_that_fails_leaves_the_session_blocked_and_free(fake):
    fake.fail = True
    session = await mini.create_session()
    after = await mini.send_message(session.id, "hi")

    events = await _read_all(session.id, after)

    assert events[-1] == {"type": "error", "message": "the agent fell over", "code": None, "details": None}
    assert session.status == "blocked"
    assert not session.busy


async def test_a_session_moves_to_another_model_and_says_so(fake):
    session = await mini.create_session()

    await mini.set_model(session.id, "other/fast")

    assert fake.model_calls == ["other/fast"]
    assert session.model == "other/fast"
    assert await _read_all(session.id) == [{"type": "model_changed", "model": "other/fast"}]


async def test_choosing_the_model_already_in_use_does_nothing(fake):
    session = await mini.create_session()

    await mini.set_model(session.id, "provider/model")

    assert fake.model_calls == []
    assert await _read_all(session.id) == []


async def test_a_model_the_harness_does_not_offer_is_refused(fake):
    session = await mini.create_session()

    with pytest.raises(ValueError, match="no model named nope"):
        await mini.set_model(session.id, "nope")

    assert session.model == "provider/model"
    assert await _read_all(session.id) == []


async def test_the_model_cannot_change_while_a_reply_is_in_flight(fake):
    fake.ask_first = True
    session = await mini.create_session()
    await mini.send_message(session.id, "hi")

    with pytest.raises(mini.MiniBusyError):
        await mini.set_model(session.id, "other/fast")

    await mini.interrupt(session.id)
    await session.task


async def test_a_new_session_opens_on_the_model_asked_for(fake):
    session = await mini.create_session(model="other/fast")

    assert session.model == "other/fast"
    assert fake.model_calls == ["other/fast"]


async def test_a_new_session_falls_back_when_the_model_asked_for_is_gone(fake):
    session = await mini.create_session(model="retired/model")

    assert session.model == "provider/model"


async def test_an_unknown_session_is_reported(fake):
    with pytest.raises(mini.MiniSessionNotFoundError):
        await mini.send_message("nope", "hi")


# ── Routes and the lock ───────────────────────────────────────────────────


def _client(host="127.0.0.1"):
    app = FastAPI()
    app.include_router(router)
    return TestClient(app, client=(host, 50000))


def _auth():
    return {"Authorization": f"Bearer {mini_auth.ensure_token()}"}


def test_the_token_is_created_once_and_private(fake, tmp_path):
    token = mini_auth.ensure_token()
    path = tmp_path / "mini-token"

    assert len(token) >= 32
    assert path.read_text() == token
    assert path.stat().st_mode & 0o777 == 0o600
    assert mini_auth.ensure_token() == token


def test_a_token_left_readable_by_others_is_locked_down(fake, tmp_path):
    path = tmp_path / "mini-token"
    path.write_text("existing-token")
    path.chmod(0o644)

    assert mini_auth.ensure_token() == "existing-token"
    assert path.stat().st_mode & 0o777 == 0o600


@pytest.mark.parametrize("headers", [{}, {"Authorization": "Bearer wrong"}, {"Authorization": "Basic abc"}])
def test_a_caller_without_the_token_is_refused(fake, headers):
    response = _client().post("/mini/sessions", json={}, headers=headers)

    assert response.status_code == 401


def test_a_caller_from_another_machine_is_refused_even_with_the_token(fake):
    response = _client(host="192.168.1.20").post("/mini/sessions", json={}, headers=_auth())

    assert response.status_code == 403


def test_harnesses_are_listed_behind_the_lock(fake):
    client = _client()

    assert client.get("/harnesses").status_code == 401
    listed = client.get("/harnesses", headers=_auth()).json()
    assert listed == [
        {
            "name": "fake",
            "display_name": "Fake",
            "installed": True,
            "binary": "/fake",
            "version": "1",
            "signed_in": None,
            "fix_command": None,
        }
    ]


def test_a_conversation_over_http(fake):
    with _client() as client:
        session = client.post("/mini/sessions", json={}, headers=_auth()).json()
        assert session["harness"] == "fake"
        assert session["status"] == "ready"

        sent = client.post(f"/mini/sessions/{session['id']}/messages", json={"text": "hi"}, headers=_auth())
        assert sent.status_code == 202

        stream = client.get(f"/mini/sessions/{session['id']}/events?after={sent.json()['after']}", headers=_auth())
        events = [json.loads(line[5:]) for line in stream.text.splitlines() if line.startswith("data:")]

        assert _types(events) == ["user_message", "status_change", "text_delta", "status_change", "message_done"]
        assert events[2]["text"] == "HI"
        assert client.get(f"/mini/sessions/{session['id']}", headers=_auth()).json()["last_seq"] == 5


def test_models_are_listed_and_switched_over_http(fake):
    with _client() as client:
        session = client.post("/mini/sessions", json={}, headers=_auth()).json()
        url = f"/mini/sessions/{session['id']}"

        listed = client.get(f"{url}/models", headers=_auth()).json()
        assert listed["current"] == "provider/model"
        assert [(model["id"], model["provider"]) for model in listed["models"]] == [
            ("provider/model", "provider"),
            ("other/fast", "other"),
        ]

        moved = client.patch(url, json={"model": "other/fast"}, headers=_auth())
        assert moved.status_code == 200
        assert moved.json()["model"] == "other/fast"
        assert client.get(f"{url}/models", headers=_auth()).json()["current"] == "other/fast"

        assert client.patch(url, json={"model": "nope"}, headers=_auth()).status_code == 400
        assert client.patch(url, json={"model": "other/fast"}).status_code == 401


def test_a_session_can_be_opened_on_a_chosen_model_over_http(fake):
    with _client() as client:
        session = client.post("/mini/sessions", json={"model": "other/fast"}, headers=_auth()).json()

    assert session["model"] == "other/fast"


def test_an_unknown_session_is_a_404(fake):
    response = _client().post("/mini/sessions/nope/messages", json={"text": "hi"}, headers=_auth())

    assert response.status_code == 404


def test_an_empty_message_is_refused(fake):
    with _client() as client:
        session = client.post("/mini/sessions", json={}, headers=_auth()).json()
        response = client.post(f"/mini/sessions/{session['id']}/messages", json={"text": ""}, headers=_auth())

    assert response.status_code == 422
