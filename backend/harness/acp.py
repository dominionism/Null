"""Drive any agent that speaks the Agent Client Protocol (ACP) over stdio.

One adapter, a row per agent: Oh-my-pi today (`omp acp`), others by adding a
row to `ACP_AGENTS`. The agent is the user's own install and holds its own
sign-in; this module starts it and translates.

Two things ACP does *not* carry over from the terminal, learned from OMP: the
agent ignores the approval mode the user configured, and it loads none of the
user's MCP servers. A row can supply both, so the agent behind the mini is the
same agent the user gets in a terminal.
"""

from __future__ import annotations

import asyncio
import contextlib
import itertools
import json
import logging
from collections.abc import AsyncIterator, Awaitable, Callable
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from acp import PROTOCOL_VERSION, RequestError, schema, spawn_agent_process, text_block

from .. import __version__
from .base import (
    ApprovalOption,
    ApprovalRequest,
    HarnessError,
    HarnessEvent,
    HarnessInfo,
    HarnessSession,
    HarnessUnavailableError,
    MessageDone,
    ModelChoice,
    StatusChange,
    StopReason,
    TextDelta,
    ToolActivity,
)
from .discovery import find_binary

logger = logging.getLogger(__name__)

#: ACP messages are one JSON document per line, and a tool's output or a
#: replayed transcript can be far longer than asyncio's 64 KiB default.
_LINE_LIMIT = 32 * 1024 * 1024

_STOP_REASONS: dict[str, StopReason] = {
    "end_turn": "completed",
    "cancelled": "cancelled",
    "max_tokens": "truncated",
    "max_turn_requests": "truncated",
    "refusal": "refused",
}

#: Marks the end of one reply on a session's event queue.
_REPLY_ENDED = object()


@dataclass(frozen=True)
class AcpAgent:
    """One row of the launch table: how to start an agent in ACP mode."""

    name: str  # registry key, e.g. "omp"
    display_name: str
    binary: str  # CLI name, found by `find_binary`
    args: tuple[str, ...]  # what puts the CLI in ACP mode
    login_command: str
    #: Extra arguments worked out at launch, given the CLI's path.
    launch_args: Callable[[str], Awaitable[list[str]]] | None = None
    #: The user's MCP servers, as `{name: entry}`, to attach to every session.
    mcp_servers: Callable[[], dict[str, dict[str, Any]]] | None = None


async def _run(binary: str, *args: str, timeout: float = 5.0) -> str | None:
    """Run the CLI for one short answer. None when it fails or stalls."""
    try:
        process = await asyncio.create_subprocess_exec(
            binary,
            *args,
            stdin=asyncio.subprocess.DEVNULL,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.DEVNULL,
        )
    except OSError:
        return None
    try:
        stdout, _ = await asyncio.wait_for(process.communicate(), timeout)
    except TimeoutError:
        process.kill()
        await process.wait()
        return None
    if process.returncode != 0:
        return None
    return stdout.decode("utf-8", "replace").strip() or None


async def _omp_launch_args(binary: str) -> list[str]:
    """Carry the approval mode the user set for the terminal into ACP."""
    mode = await _run(binary, "config", "get", "tools.approvalMode")
    return ["--approval-mode", mode] if mode in ("always-ask", "write", "yolo") else []


def _omp_mcp_servers() -> dict[str, dict[str, Any]]:
    """The MCP servers OMP loads in a terminal."""
    path = Path("~/.omp/agent/mcp.json").expanduser()
    try:
        servers = json.loads(path.read_text()).get("mcpServers")
    except (OSError, ValueError, AttributeError):
        return {}
    return servers if isinstance(servers, dict) else {}


ACP_AGENTS: dict[str, AcpAgent] = {
    "omp": AcpAgent(
        name="omp",
        display_name="Oh-my-pi",
        binary="omp",
        args=("acp",),
        login_command="omp login",
        launch_args=_omp_launch_args,
        mcp_servers=_omp_mcp_servers,
    ),
}


def mcp_server_from_config(name: str, entry: dict[str, Any], capabilities: Any = None) -> Any | None:
    """Turn one `mcp.json` entry into what `session/new` accepts.

    Returns None for an entry the agent cannot take: an unknown shape, or a
    transport the agent did not say it supports.
    """
    if entry.get("command"):
        return schema.McpServerStdio(
            name=name,
            command=str(entry["command"]),
            args=[str(arg) for arg in entry.get("args") or []],
            env=[schema.EnvVariable(name=key, value=str(value)) for key, value in (entry.get("env") or {}).items()],
        )

    transport = entry.get("type") or ("http" if entry.get("url") else None)
    if transport not in ("http", "sse") or not entry.get("url"):
        return None
    if not getattr(capabilities, transport, False):
        return None
    headers = [schema.HttpHeader(name=key, value=str(value)) for key, value in (entry.get("headers") or {}).items()]
    server = schema.HttpMcpServer if transport == "http" else schema.SseMcpServer
    return server(type=transport, name=name, url=str(entry["url"]), headers=headers)


def event_from_update(update: Any, tools: dict[str, ToolActivity]) -> HarnessEvent | None:
    """Translate one `session/update` into an event, or None when the mini has no use for it.

    `tools` remembers each tool call, because a progress update names only what changed.
    """
    if isinstance(update, schema.AgentMessageChunk | schema.AgentThoughtChunk):
        text = getattr(update.content, "text", None)
        if not text:
            return None
        return TextDelta(text, thinking=isinstance(update, schema.AgentThoughtChunk))

    if isinstance(update, schema.ToolCallStart | schema.ToolCallProgress):
        known = tools.get(update.tool_call_id)
        activity = ToolActivity(
            call_id=update.tool_call_id,
            title=update.title or (known.title if known else "Tool"),
            status=update.status or (known.status if known else "pending"),
            kind=update.kind or (known.kind if known else None),
            raw_input=update.raw_input if update.raw_input is not None else (known.raw_input if known else None),
            raw_output=update.raw_output if update.raw_output is not None else (known.raw_output if known else None),
        )
        tools[update.tool_call_id] = activity
        return activity

    return None


def models_from_config_options(options: list[Any]) -> tuple[list[ModelChoice], str | None]:
    """Pull the model list and the current model out of a session's config options."""
    for option in options:
        if getattr(option, "id", None) != "model" and getattr(option, "category", None) != "model":
            continue
        choices: list[ModelChoice] = []
        for entry in getattr(option, "options", None) or []:
            grouped = getattr(entry, "options", None)
            if grouped is None:
                provider = entry.value.split("/", 1)[0] if "/" in entry.value else None
                choices.append(ModelChoice(id=entry.value, label=entry.name, provider=provider))
                continue
            choices.extend(ModelChoice(id=choice.value, label=choice.name, provider=entry.name) for choice in grouped)
        return choices, getattr(option, "current_value", None)
    return [], None


@dataclass
class _Session:
    cwd: str
    process_number: int  # which launch of the agent holds this session in memory
    config_options: list[Any] = field(default_factory=list)
    queue: asyncio.Queue[Any] | None = None  # set while a reply is in flight
    tools: dict[str, ToolActivity] = field(default_factory=dict)
    pending: dict[str, asyncio.Future[str | None]] = field(default_factory=dict)  # approvals awaiting an answer


class _Client:
    """What the agent may ask of Null: report progress, and ask permission.

    Null offers the agent no file system or terminal of its own; the agent uses its own tools.
    """

    def __init__(self, harness: AcpHarness) -> None:
        self._harness = harness

    def on_connect(self, conn: Any) -> None:
        pass

    async def session_update(self, session_id: str, update: Any, **kwargs: Any) -> None:
        self._harness._on_update(session_id, update)

    async def request_permission(
        self, session_id: str, tool_call: Any, options: list[Any], **kwargs: Any
    ) -> schema.RequestPermissionResponse:
        return await self._harness._on_permission(session_id, tool_call, options)


class AcpHarness:
    """One ACP agent process, serving every session Null opens on it."""

    def __init__(self, agent: AcpAgent, *, binary: str | None = None) -> None:
        self._agent = agent
        self._binary_override = binary
        self._lock = asyncio.Lock()
        self._stack: contextlib.AsyncExitStack | None = None
        self._conn: Any = None
        self._process: Any = None
        self._process_number = 0
        self._stderr_task: asyncio.Task[None] | None = None
        self._stderr_tail: list[str] = []
        self._mcp_capabilities: Any = None
        self._sessions: dict[str, _Session] = {}
        self._request_ids = itertools.count(1)

    # ── Harness protocol ──────────────────────────────────────────────────

    async def info(self) -> HarnessInfo:
        binary = find_binary(self._agent.binary, override=self._binary_override)
        if binary is None:
            return HarnessInfo(name=self._agent.name, display_name=self._agent.display_name, installed=False)
        return HarnessInfo(
            name=self._agent.name,
            display_name=self._agent.display_name,
            installed=True,
            binary=binary,
            version=await _run(binary, "--version"),
        )

    async def start_session(
        self,
        cwd: str,
        *,
        resume: str | None = None,
        model: str | None = None,
    ) -> HarnessSession:
        conn = await self._connect()
        servers = self._mcp_servers()
        if resume:
            response = await conn.load_session(cwd=cwd, session_id=resume, mcp_servers=servers)
            session_id = resume
        else:
            response = await conn.new_session(cwd=cwd, mcp_servers=servers)
            session_id = response.session_id

        session = _Session(cwd=cwd, process_number=self._process_number)
        session.config_options = list(getattr(response, "config_options", None) or [])
        self._sessions[session_id] = session
        if model:
            await self.set_model(session_id, model)

        _, current = models_from_config_options(session.config_options)
        return HarnessSession(session_id=session_id, cwd=cwd, model=current)

    async def send(self, session_id: str, text: str) -> AsyncIterator[HarnessEvent]:
        session = self._session(session_id)
        if session.queue is not None:
            raise RuntimeError(f"Session {session_id} is still answering the previous message")

        queue: asyncio.Queue[Any] = asyncio.Queue()
        session.queue = queue
        prompt: asyncio.Task[Any] | None = None
        try:
            yield StatusChange("running")
            conn = await self._reattach(session_id, session)
            prompt = asyncio.create_task(conn.prompt(session_id=session_id, prompt=[text_block(text)]))
            prompt.add_done_callback(lambda _: queue.put_nowait(_REPLY_ENDED))

            while (event := await queue.get()) is not _REPLY_ENDED:
                yield event
            while not queue.empty():
                yield queue.get_nowait()

            response = prompt.result()
            yield StatusChange("ready")
            yield MessageDone(_STOP_REASONS.get(response.stop_reason, "completed"))
        except RequestError as exc:
            data = exc.data if isinstance(exc.data, dict) else {}
            details = data.get("details") or (str(exc.data) if exc.data else None)
            yield StatusChange("blocked", str(exc))
            yield HarnessError(message=str(exc), code=exc.code, details=details)
        except (HarnessUnavailableError, OSError, ConnectionError) as exc:
            yield StatusChange("blocked", str(exc))
            yield HarnessError(message=str(exc), details="\n".join(self._stderr_tail[-5:]) or None)
        finally:
            session.queue = None
            self._release_pending(session)
            if prompt is not None and not prompt.done():
                # Whoever was reading went away; do not leave the agent working for nobody.
                prompt.cancel()
                with contextlib.suppress(Exception):
                    await self._conn.cancel(session_id=session_id)

    async def respond(self, session_id: str, request_id: str, answer: str) -> None:
        future = self._session(session_id).pending.get(request_id)
        if future is None or future.done():
            raise ValueError(f"No approval request {request_id} is waiting on session {session_id}")
        future.set_result(answer)

    async def interrupt(self, session_id: str) -> None:
        session = self._session(session_id)
        # The protocol requires every open permission request to be answered "cancelled".
        self._release_pending(session)
        if self._conn is not None and session.queue is not None:
            await self._conn.cancel(session_id=session_id)

    async def list_models(self, session_id: str | None = None) -> list[ModelChoice]:
        if session_id is not None:
            sessions = [self._session(session_id)]
        else:
            sessions = list(reversed(self._sessions.values()))
        for session in sessions:
            choices, _ = models_from_config_options(session.config_options)
            if choices:
                return choices
        return []

    async def set_model(self, session_id: str, model_id: str) -> None:
        session = self._session(session_id)
        choices, _ = models_from_config_options(session.config_options)
        if choices and model_id not in {choice.id for choice in choices}:
            raise ValueError(f"{self._agent.display_name} has no model named {model_id}")
        conn = await self._reattach(session_id, session)
        try:
            response = await conn.set_config_option(config_id="model", session_id=session_id, value=model_id)
        except RequestError as exc:
            data = exc.data if isinstance(exc.data, dict) else {}
            raise ValueError(data.get("details") or str(exc)) from exc
        options = getattr(response, "config_options", None)
        if options:
            session.config_options = list(options)

    async def close(self) -> None:
        async with self._lock:
            await self._shutdown()
            self._sessions.clear()

    # ── Process ───────────────────────────────────────────────────────────

    async def _connect(self) -> Any:
        """The connection to the agent, starting or restarting the process when needed."""
        async with self._lock:
            if self._conn is not None and self._process.returncode is None:
                return self._conn
            await self._shutdown()

            binary = find_binary(self._agent.binary, override=self._binary_override)
            if binary is None:
                raise HarnessUnavailableError(f"{self._agent.display_name} is not installed")
            extra = await self._agent.launch_args(binary) if self._agent.launch_args else []

            stack = contextlib.AsyncExitStack()
            try:
                conn, process = await stack.enter_async_context(
                    spawn_agent_process(
                        _Client(self),
                        binary,
                        *extra,
                        *self._agent.args,
                        transport_kwargs={"limit": _LINE_LIMIT},
                    )
                )
                self._stderr_task = asyncio.create_task(self._drain_stderr(process))
                init = await conn.initialize(
                    protocol_version=PROTOCOL_VERSION,
                    client_capabilities=schema.ClientCapabilities(),
                    client_info=schema.Implementation(name="null-mini", title="Null Mini", version=__version__),
                )
                if init.protocol_version != PROTOCOL_VERSION:
                    raise HarnessUnavailableError(
                        f"{self._agent.display_name} speaks ACP version {init.protocol_version}; "
                        f"Null speaks version {PROTOCOL_VERSION}"
                    )
            except BaseException:
                await stack.aclose()
                raise

            capabilities = getattr(init, "agent_capabilities", None)
            self._mcp_capabilities = getattr(capabilities, "mcp_capabilities", None)
            self._stack, self._conn, self._process = stack, conn, process
            self._process_number += 1
            agent_info = getattr(init, "agent_info", None)
            logger.info(
                "Harness %s started (%s %s, pid %s, args %s)",
                self._agent.name,
                getattr(agent_info, "name", "?"),
                getattr(agent_info, "version", "?"),
                process.pid,
                [*extra, *self._agent.args],
            )
            return conn

    async def _reattach(self, session_id: str, session: _Session) -> Any:
        """The connection, with `session` loaded again if the agent was restarted since it was opened."""
        conn = await self._connect()
        if session.process_number != self._process_number:
            response = await conn.load_session(cwd=session.cwd, session_id=session_id, mcp_servers=self._mcp_servers())
            session.config_options = list(getattr(response, "config_options", None) or session.config_options)
            session.process_number = self._process_number
        return conn

    async def _shutdown(self) -> None:
        for session in self._sessions.values():
            self._release_pending(session)
        if self._stderr_task is not None:
            self._stderr_task.cancel()
            self._stderr_task = None
        stack, self._stack, self._conn, self._process = self._stack, None, None, None
        if stack is not None:
            with contextlib.suppress(Exception):
                await stack.aclose()

    async def _drain_stderr(self, process: Any) -> None:
        """Keep the agent's stderr pipe empty so it can never block, and keep the last lines for errors."""
        if process.stderr is None:
            return
        async for raw in process.stderr:
            self._stderr_tail.append(raw.decode("utf-8", "replace").rstrip())
            del self._stderr_tail[:-20]

    # ── Calls from the agent ──────────────────────────────────────────────

    def _on_update(self, session_id: str, update: Any) -> None:
        session = self._sessions.get(session_id)
        if session is None:
            return
        if isinstance(update, schema.ConfigOptionUpdate):
            session.config_options = list(update.config_options)
            return
        event = event_from_update(update, session.tools)
        if event is not None and session.queue is not None:
            session.queue.put_nowait(event)

    async def _on_permission(
        self, session_id: str, tool_call: Any, options: list[Any]
    ) -> schema.RequestPermissionResponse:
        denied = schema.RequestPermissionResponse(outcome=schema.DeniedOutcome(outcome="cancelled"))
        session = self._sessions.get(session_id)
        if session is None or session.queue is None:
            return denied

        request_id = f"approval-{next(self._request_ids)}"
        known = session.tools.get(tool_call.tool_call_id)
        future: asyncio.Future[str | None] = asyncio.get_running_loop().create_future()
        session.pending[request_id] = future
        session.queue.put_nowait(StatusChange("needs_input"))
        session.queue.put_nowait(
            ApprovalRequest(
                request_id=request_id,
                title=tool_call.title or (known.title if known else "Permission needed"),
                options=tuple(ApprovalOption(option.option_id, option.name, option.kind) for option in options),
                call_id=tool_call.tool_call_id,
            )
        )
        try:
            answer = await future
        finally:
            session.pending.pop(request_id, None)

        if answer is None:
            return denied
        if session.queue is not None:
            session.queue.put_nowait(StatusChange("running"))
        return schema.RequestPermissionResponse(outcome=schema.AllowedOutcome(option_id=answer, outcome="selected"))

    # ── Helpers ───────────────────────────────────────────────────────────

    def _session(self, session_id: str) -> _Session:
        session = self._sessions.get(session_id)
        if session is None:
            raise ValueError(f"Unknown session: {session_id}")
        return session

    def _mcp_servers(self) -> list[Any]:
        if self._agent.mcp_servers is None:
            return []
        servers = []
        for name, entry in self._agent.mcp_servers().items():
            server = mcp_server_from_config(name, entry, self._mcp_capabilities) if isinstance(entry, dict) else None
            if server is None:
                logger.warning("Harness %s: MCP server %r cannot be attached over ACP; skipped", self._agent.name, name)
                continue
            servers.append(server)
        return servers

    @staticmethod
    def _release_pending(session: _Session) -> None:
        for future in session.pending.values():
            if not future.done():
                future.set_result(None)
