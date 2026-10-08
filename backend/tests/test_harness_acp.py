"""Tests for the ACP adapter: what it makes of what an agent reports.

No agent is started. The translations are tested directly, and a fake
connection stands in for the agent process to pin the reply loop: ordering,
approvals, interruption, and what a failure becomes.
"""

import asyncio
from types import SimpleNamespace

import pytest
from acp import (
    RequestError,
    schema,
    start_tool_call,
    update_agent_message_text,
    update_agent_thought_text,
    update_tool_call,
)

from backend.harness import (
    ApprovalRequest,
    HarnessError,
    MessageDone,
    ModelChoice,
    StatusChange,
    TextDelta,
    ToolActivity,
)
from backend.harness.acp import (
    ACP_AGENTS,
    AcpHarness,
    event_from_update,
    mcp_server_from_config,
    models_from_config_options,
)


def _model_option(current="go/flash"):
    return schema.SessionConfigOptionSelect(
        id="model",
        name="Model",
        type="select",
        current_value=current,
        options=[
            schema.SessionConfigSelectGroup(
                group="go",
                name="opencode-go",
                options=[schema.SessionConfigSelectOption(value="go/flash", name="Flash")],
            ),
            schema.SessionConfigSelectGroup(
                group="codex",
                name="openai-codex",
                options=[schema.SessionConfigSelectOption(value="codex/luna", name="Luna")],
            ),
        ],
    )


class FakeConn:
    """Stands in for the agent: `script` plays the agent's side of one reply."""

    def __init__(self):
        self.script = None
        self.cancelled = []
        self.set_calls = []
        self.loaded = []
        self.stop = asyncio.Event()

    async def new_session(self, cwd, mcp_servers=None):
        return SimpleNamespace(session_id="s1", config_options=[_model_option()])

    async def load_session(self, cwd, session_id, mcp_servers=None):
        self.loaded.append(session_id)
        return SimpleNamespace(config_options=[_model_option()])

    async def prompt(self, session_id, prompt):
        return await self.script(session_id, prompt[0].text)

    async def cancel(self, session_id):
        self.cancelled.append(session_id)
        self.stop.set()

    async def set_config_option(self, config_id, session_id, value):
        self.set_calls.append((config_id, session_id, value))
        return SimpleNamespace(config_options=[_model_option(current=value)])


@pytest.fixture
def agent(monkeypatch):
    """An adapter whose agent process is a fake connection."""
    harness = AcpHarness(ACP_AGENTS["omp"])
    conn = FakeConn()

    async def connect():
        harness._conn = conn
        harness._process_number = max(harness._process_number, 1)
        return conn

    monkeypatch.setattr(harness, "_connect", connect)
    monkeypatch.setattr(harness, "_mcp_servers", list)
    return SimpleNamespace(harness=harness, conn=conn)


async def _collect(harness, session_id, text):
    return [event async for event in harness.send(session_id, text)]


# ── Translations ──────────────────────────────────────────────────────────


def test_answer_text_and_thinking_text_are_told_apart():
    assert event_from_update(update_agent_message_text("hi"), {}) == TextDelta("hi")
    assert event_from_update(update_agent_thought_text("hmm"), {}) == TextDelta("hmm", thinking=True)


def test_a_progress_update_keeps_what_the_tool_call_started_with():
    tools = {}
    started = event_from_update(
        start_tool_call("c1", "$ ls", kind="execute", status="pending", raw_input={"command": "ls"}), tools
    )
    finished = event_from_update(update_tool_call("c1", status="completed", raw_output="a.txt"), tools)

    assert started == ToolActivity("c1", "$ ls", "pending", kind="execute", raw_input={"command": "ls"})
    assert finished == ToolActivity(
        "c1", "$ ls", "completed", kind="execute", raw_input={"command": "ls"}, raw_output="a.txt"
    )


def test_an_update_the_mini_has_no_use_for_is_dropped():
    usage = schema.UsageUpdate(session_update="usage_update", used=10, size=100)

    assert event_from_update(usage, {}) is None


def test_models_are_listed_with_their_provider_and_the_current_one():
    choices, current = models_from_config_options([_model_option()])

    assert choices == [
        ModelChoice(id="go/flash", label="Flash", provider="opencode-go"),
        ModelChoice(id="codex/luna", label="Luna", provider="openai-codex"),
    ]
    assert current == "go/flash"


def test_a_session_without_a_model_option_lists_nothing():
    assert models_from_config_options([]) == ([], None)


def test_an_http_mcp_server_carries_its_headers():
    capabilities = schema.McpCapabilities(http=True, sse=False)

    server = mcp_server_from_config(
        "voicebox", {"type": "http", "url": "http://127.0.0.1:17493/mcp", "headers": {"X-Id": "omp"}}, capabilities
    )

    assert isinstance(server, schema.HttpMcpServer)
    assert (server.name, server.url) == ("voicebox", "http://127.0.0.1:17493/mcp")
    assert [(header.name, header.value) for header in server.headers] == [("X-Id", "omp")]


def test_a_transport_the_agent_does_not_support_is_left_out():
    capabilities = schema.McpCapabilities(http=True, sse=False)

    assert mcp_server_from_config("old", {"type": "sse", "url": "http://x/sse"}, capabilities) is None
    assert mcp_server_from_config("odd", {"type": "carrier-pigeon"}, capabilities) is None


def test_a_command_mcp_server_carries_its_arguments_and_environment():
    server = mcp_server_from_config("files", {"command": "npx", "args": ["-y", "fs"], "env": {"ROOT": "/tmp"}})

    assert isinstance(server, schema.McpServerStdio)
    assert (server.command, server.args) == ("npx", ["-y", "fs"])
    assert [(variable.name, variable.value) for variable in server.env] == [("ROOT", "/tmp")]


# ── The reply loop ────────────────────────────────────────────────────────


async def test_a_session_starts_on_the_agents_current_model(agent):
    session = await agent.harness.start_session("/work")

    assert (session.session_id, session.cwd, session.model) == ("s1", "/work", "go/flash")
    assert len(await agent.harness.list_models("s1")) == 2


async def test_a_reply_is_reported_in_the_order_the_agent_sent_it(agent):
    harness = agent.harness

    async def script(session_id, text):
        harness._on_update(session_id, update_agent_thought_text("thinking"))
        harness._on_update(session_id, start_tool_call("c1", "$ ls", kind="execute", status="in_progress"))
        harness._on_update(session_id, update_tool_call("c1", status="completed"))
        harness._on_update(session_id, update_agent_message_text(text.upper()))
        return SimpleNamespace(stop_reason="end_turn")

    agent.conn.script = script
    await harness.start_session("/work")

    events = await _collect(harness, "s1", "hi")

    assert events == [
        StatusChange("running"),
        TextDelta("thinking", thinking=True),
        ToolActivity("c1", "$ ls", "in_progress", kind="execute"),
        ToolActivity("c1", "$ ls", "completed", kind="execute"),
        TextDelta("HI"),
        StatusChange("ready"),
        MessageDone("completed"),
    ]


async def test_an_approval_waits_for_the_answer_and_passes_it_to_the_agent(agent):
    harness = agent.harness
    outcomes = []

    async def script(session_id, text):
        harness._on_update(session_id, start_tool_call("c1", "$ rm -rf build", kind="execute"))
        response = await harness._on_permission(
            session_id,
            schema.ToolCallUpdate(tool_call_id="c1"),
            [schema.PermissionOption(option_id="o1", name="Allow once", kind="allow_once")],
        )
        outcomes.append(response.outcome)
        return SimpleNamespace(stop_reason="end_turn")

    agent.conn.script = script
    await harness.start_session("/work")

    seen = []
    async for event in harness.send("s1", "clean up"):
        seen.append(event)
        if isinstance(event, ApprovalRequest):
            assert event.title == "$ rm -rf build"
            assert [(option.option_id, option.kind) for option in event.options] == [("o1", "allow_once")]
            await harness.respond("s1", event.request_id, "o1")

    assert [type(event).__name__ for event in seen] == [
        "StatusChange",
        "ToolActivity",
        "StatusChange",
        "ApprovalRequest",
        "StatusChange",
        "StatusChange",
        "MessageDone",
    ]
    assert [event.status for event in seen if isinstance(event, StatusChange)] == [
        "running",
        "needs_input",
        "running",
        "ready",
    ]
    assert (outcomes[0].outcome, outcomes[0].option_id) == ("selected", "o1")


async def test_an_interrupt_cancels_the_agent_and_any_open_approval(agent):
    harness = agent.harness
    outcomes = []

    async def script(session_id, text):
        response = await harness._on_permission(
            session_id,
            schema.ToolCallUpdate(tool_call_id="c1", title="$ sleep 999"),
            [schema.PermissionOption(option_id="o1", name="Allow once", kind="allow_once")],
        )
        outcomes.append(response.outcome.outcome)
        await agent.conn.stop.wait()
        return SimpleNamespace(stop_reason="cancelled")

    agent.conn.script = script
    await harness.start_session("/work")

    seen = []
    async for event in harness.send("s1", "wait"):
        seen.append(event)
        if isinstance(event, ApprovalRequest):
            await harness.interrupt("s1")

    assert outcomes == ["cancelled"]
    assert agent.conn.cancelled == ["s1"]
    assert seen[-1] == MessageDone("cancelled")


async def test_an_agent_error_becomes_an_error_event_and_frees_the_session(agent):
    harness = agent.harness

    async def fail(session_id, text):
        raise RequestError(-32603, "Internal error", {"details": "provider said no"})

    async def answer(session_id, text):
        return SimpleNamespace(stop_reason="end_turn")

    agent.conn.script = fail
    await harness.start_session("/work")

    events = await _collect(harness, "s1", "hi")

    assert events == [
        StatusChange("running"),
        StatusChange("blocked", "Internal error"),
        HarnessError("Internal error", code=-32603, details="provider said no"),
    ]
    agent.conn.script = answer
    assert (await _collect(harness, "s1", "again"))[-1] == MessageDone("completed")


async def test_a_second_message_is_refused_while_the_first_is_being_answered(agent):
    harness = agent.harness

    async def script(session_id, text):
        await agent.conn.stop.wait()
        return SimpleNamespace(stop_reason="cancelled")

    agent.conn.script = script
    await harness.start_session("/work")
    first = harness.send("s1", "one")
    await anext(first)

    with pytest.raises(RuntimeError, match="still answering"):
        await _collect(harness, "s1", "two")

    await harness.interrupt("s1")
    assert [event async for event in first][-1] == MessageDone("cancelled")


async def test_changing_the_model_tells_the_agent_and_remembers_the_answer(agent):
    harness = agent.harness
    await harness.start_session("/work")

    await harness.set_model("s1", "codex/luna")

    assert agent.conn.set_calls == [("model", "s1", "codex/luna")]
    assert models_from_config_options(harness._sessions["s1"].config_options)[1] == "codex/luna"


async def test_a_session_is_loaded_again_after_the_agent_restarts(agent):
    harness = agent.harness

    async def script(session_id, text):
        return SimpleNamespace(stop_reason="end_turn")

    agent.conn.script = script
    await harness.start_session("/work")
    harness._process_number += 1  # the agent process was replaced

    await _collect(harness, "s1", "hi")
    await _collect(harness, "s1", "again")

    assert agent.conn.loaded == ["s1"]


async def test_an_unknown_session_is_refused(agent):
    with pytest.raises(ValueError, match="Unknown session"):
        await _collect(agent.harness, "nope", "hi")
