"""End-to-end tests for the narration lane HTTP surface.

Exercises ``POST /speak/narrate`` and ``GET /speak/{id}/stream`` against the real
FastAPI app over an in-process ASGI transport, with the TTS engine stubbed out —
no model download, no GPU, no network. What is *not* stubbed is everything the
narration lane is responsible for: lane resolution, session lifetime, chunk
streaming, cancellation, the event schema, and the fact that narration never
touches the serial generation queue.
"""

import asyncio
import base64
import io
import json

import numpy as np
import pytest
import soundfile as sf
from httpx import AsyncClient

from backend.database import get_db
from backend.mcp_server import events as mcp_events
from backend.services import narration

SAMPLE_RATE = 24000
# Longer than DEFAULT_MAX_CHUNK_CHARS (800), so the utterance splits into several
# chunks — incremental delivery is exactly what these tests prove. A short line
# stays one chunk on purpose: the engine keeps its cross-sentence prosody.
NARRATION_TEXT = (
    "Reading the auth middleware now. "
    "The token check happens before the handler runs. "
    "That looks like the source of the failure. "
) * 7


class StubNarrationBackend:
    """
    Minimal TTS backend: a deterministic sine per chunk.

    ``gate`` blocks every synthesis call after the first until it is set, which
    lets a test prove the client received chunk 0 while the server was still
    synthesizing chunk 1 — deterministically, with no timing assumptions.
    """

    def __init__(self) -> None:
        self.started_calls = 0
        self.completed_calls = 0
        self.gate: asyncio.Event | None = None

    async def load_model(self, model_size: str = "default") -> None:
        return None

    def unload_model(self) -> None:
        return None

    async def generate(
        self,
        text: str,
        voice_prompt: dict,
        language: str = "en",
        seed: int | None = None,
        instruct: str | None = None,
    ) -> tuple[np.ndarray, int]:
        self.started_calls += 1
        if self.gate is not None and self.started_calls > 1:
            await self.gate.wait()

        samples = max(1, len(text) * 40)
        ramp = np.arange(samples, dtype=np.float32)
        audio = (0.4 * np.sin(ramp / 11.0)).astype(np.float32)
        self.completed_calls += 1
        return audio, SAMPLE_RATE


class _EventCollector:
    """Subscribes to the in-process speak event bus for the duration of a test."""

    def __init__(self) -> None:
        self.queue = mcp_events.subscribe()

    def drain(self) -> list[dict]:
        collected = []
        while not self.queue.empty():
            collected.append(self.queue.get_nowait())
        return collected

    def close(self) -> None:
        mcp_events.unsubscribe(self.queue)


@pytest.fixture
def stub_backend(monkeypatch) -> StubNarrationBackend:
    """Stub the shared engine registry — narration uses the same instance as the queue."""
    import backend.backends as backends

    stub = StubNarrationBackend()
    monkeypatch.setattr(backends, "get_tts_backend_for_engine", lambda engine: stub)

    async def _load(engine: str, model_size: str = "default") -> None:
        return None

    monkeypatch.setattr(backends, "load_engine_model", _load)
    return stub


@pytest.fixture
def queue_spy(monkeypatch):
    """
    Records every enqueue on the serial generation queue.

    The spy closes the job coroutine instead of running it — these tests assert
    *whether* the narration lane queued work, not what the queue then does.
    """
    import backend.routes.generations as generations
    import backend.services.task_queue as task_queue

    calls: list[str] = []

    def spy(generation_id, coro):
        calls.append(generation_id)
        coro.close()
        return

    monkeypatch.setattr(task_queue, "enqueue_generation", spy)
    monkeypatch.setattr(generations, "enqueue_generation", spy)
    return calls


@pytest.fixture
async def app_env(tmp_path, monkeypatch):
    """Point the app at a throwaway data dir and initialize the database."""
    monkeypatch.setenv("VOICEBOX_OFFLINE_PATCH", "0")
    # Never spawn the narration worker from a test: the lifespan starts one, and
    # these tests exercise the in-process path with a stubbed engine.
    monkeypatch.setenv("VOICEBOX_NARRATION_WORKER", "0")
    from backend import config, database

    config.set_data_dir(tmp_path)
    database.init_db()

    from backend.app import app

    return app


@pytest.fixture
async def client(app_env):
    """
    HTTP client against a real uvicorn server on a loopback port.

    Deliberately not ``ASGITransport``: the two behaviours this file exists to
    prove — that a chunk reaches the client before the utterance finishes
    synthesizing, and that disconnecting cancels the narration — only exist over
    a real socket with a real disconnect.
    """
    import uvicorn

    server = uvicorn.Server(
        uvicorn.Config(app_env, host="127.0.0.1", port=0, log_level="warning")
    )
    server_task = asyncio.create_task(server.serve())
    try:
        while not server.started:
            await asyncio.sleep(0.01)
        port = server.servers[0].sockets[0].getsockname()[1]

        async with AsyncClient(base_url=f"http://127.0.0.1:{port}") as http:
            # Wait for the app lifespan to finish: startup initializes the
            # generation queue, and tests mutate that state.
            for _ in range(200):
                if (await http.get("/health")).status_code == 200:
                    break
                await asyncio.sleep(0.05)
            yield http
    finally:
        server.should_exit = True
        await server_task


def create_kokoro_preset_profile(name: str = "Narrator") -> str:
    """Insert a preset Kokoro profile straight into the DB and return its id."""
    import uuid

    from backend.database import get_db
    from backend.database.models import VoiceProfile

    db = next(get_db())
    try:
        profile = VoiceProfile(
            id=str(uuid.uuid4()),
            name=name,
            language="en",
            voice_type="preset",
            preset_engine="kokoro",
            preset_voice_id="am_adam",
        )
        db.add(profile)
        db.commit()
        return profile.id
    finally:
        db.close()


def parse_sse(raw: str) -> tuple[str, dict]:
    """Parse one SSE frame (``event:`` + ``data:`` lines) into (event, payload)."""
    event = "message"
    data = "{}"
    for line in raw.splitlines():
        if line.startswith("event:"):
            event = line[len("event:") :].strip()
        elif line.startswith("data:"):
            data = line[len("data:") :].strip()
    return event, json.loads(data)


async def iter_events(response):
    """Yield parsed SSE frames from a streaming httpx response.

    sse-starlette separates frames with CRLF, so normalize before splitting.
    """
    buffer = ""
    async for text in response.aiter_text():
        buffer += text.replace("\r\n", "\n")
        while "\n\n" in buffer:
            raw, buffer = buffer.split("\n\n", 1)
            if raw.strip():
                yield parse_sse(raw)


async def test_narrate_returns_a_streaming_session(client, stub_backend, queue_spy):
    profile_id = create_kokoro_preset_profile()

    response = await client.post(
        "/speak/narrate", json={"text": NARRATION_TEXT, "profile": profile_id}
    )
    assert response.status_code == 200, response.text
    body = response.json()

    assert body["mode"] == "narration"
    assert body["engine"] == "kokoro"
    assert body["stream_url"] == f"/speak/{body['narration_id']}/stream"
    assert body["generation_id"] is None

    # The narration lane must not queue work on the serial GPU path.
    assert queue_spy == []


async def test_narrate_falls_back_when_agent_narration_is_off(client, stub_backend, queue_spy):
    """With the feature switched off, narrate degrades to the queued speak path."""
    from backend.services import settings as settings_service

    create_kokoro_preset_profile()
    settings_service.update_generation_settings(next(get_db()), {"agent_narration": False})

    response = await client.post(
        "/speak/narrate", json={"text": "Queued please.", "profile": "Narrator"}
    )
    assert response.status_code == 200, response.text
    body = response.json()

    assert body["mode"] == "generation"
    assert body["generation_id"]
    assert body["poll_url"].endswith("/status")
    assert queue_spy == [body["generation_id"]]


async def wait_for_session_gone(narration_id: str, timeout: float = 5.0) -> None:
    """Wait for the server to notice a disconnect and discard the session.

    Disconnect detection is asynchronous: the server only observes the closed
    socket once its SSE machinery reacts, so the client must give it a moment
    rather than assert immediately.
    """
    loop = asyncio.get_running_loop()
    deadline = loop.time() + timeout
    while narration.get_session(narration_id) is not None and loop.time() < deadline:
        await asyncio.sleep(0.01)


async def test_narration_stream_emits_ordered_wav_chunks_incrementally(
    client, stub_backend, queue_spy
):
    profile_id = create_kokoro_preset_profile()
    collector = _EventCollector()
    # Chunk 1 onward blocks until the client has seen chunk 0.
    stub_backend.gate = asyncio.Event()
    try:
        start = await client.post(
            "/speak/narrate", json={"text": NARRATION_TEXT, "profile": profile_id}
        )
        narration_id = start.json()["narration_id"]

        events: list[tuple[str, dict]] = []
        first_chunk_while_synthesizing = False

        async with client.stream("GET", f"/speak/{narration_id}/stream") as response:
            assert response.status_code == 200
            async for event, payload in iter_events(response):
                events.append((event, payload))
                if event == "chunk" and payload["index"] == 0:
                    # The point of streaming: chunk 0 reached the client while
                    # the server was still inside the synthesis of chunk 1.
                    first_chunk_while_synthesizing = (
                        stub_backend.completed_calls == 1 and stub_backend.started_calls == 2
                    )
                    stub_backend.gate.set()

        kinds = [event for event, _payload in events]
        assert kinds[0] == "ready"
        assert kinds[-1] == "done"

        chunk_payloads = [payload for event, payload in events if event == "chunk"]
        assert len(chunk_payloads) >= 2, "expected the text to split into several chunks"
        assert [payload["index"] for payload in chunk_payloads] == list(range(len(chunk_payloads)))

        for payload in chunk_payloads:
            audio, sample_rate = sf.read(io.BytesIO(base64.b64decode(payload["wav"])))
            assert sample_rate == SAMPLE_RATE
            assert len(audio) > 0
            assert payload["text"]
            # Narration must be as loud as normal speech: the queued path
            # normalizes to -20 dBFS RMS and the stream normalizes per chunk
            # (without this it came out ~20 dB quieter for LuxTTS).
            rms_dbfs = 20 * np.log10(np.sqrt(np.mean(audio**2)) + 1e-12)
            assert -23.0 < rms_dbfs < -17.0, f"chunk loudness {rms_dbfs:.1f} dBFS is off target"

        assert events[-1][1]["chunks"] == len(chunk_payloads)
        assert first_chunk_while_synthesizing, "chunk 0 was not delivered until synthesis finished"

        # Sessions are ephemeral, and the lane never touched the queue.
        assert narration.get_session(narration_id) is None
        assert queue_spy == []

        published = [event for event in collector.drain() if event["kind"] == "speak-end"]
        assert published, "completing a narration must publish speak-end"
        assert published[-1]["status"] == "completed"
        assert published[-1]["generation_id"] == narration_id
    finally:
        collector.close()


async def test_stream_cancellation_discards_the_session(client, stub_backend, queue_spy):
    profile_id = create_kokoro_preset_profile()
    collector = _EventCollector()
    # Block synthesis after chunk 0 so the disconnect lands mid-narration.
    stub_backend.gate = asyncio.Event()
    try:
        start = await client.post(
            "/speak/narrate", json={"text": NARRATION_TEXT, "profile": profile_id}
        )
        narration_id = start.json()["narration_id"]

        async with client.stream("GET", f"/speak/{narration_id}/stream") as response:
            assert response.status_code == 200
            async for event, _payload in iter_events(response):
                if event == "chunk":
                    break  # walk away mid-narration

        # Closing the stream is the cancel path: the session goes away and the
        # backend reports the cancellation to the pill.
        await wait_for_session_gone(narration_id)
        assert narration.get_session(narration_id) is None

        published = [event for event in collector.drain() if event["kind"] == "speak-end"]
        assert published, "cancelling a narration must publish speak-end"
        assert published[-1]["status"] == "cancelled"
        assert published[-1]["generation_id"] == narration_id
    finally:
        collector.close()


async def test_second_listener_is_rejected(client, stub_backend, queue_spy):
    profile_id = create_kokoro_preset_profile()
    # Hold the stream open so the first listener still owns the session.
    stub_backend.gate = asyncio.Event()

    start = await client.post("/speak/narrate", json={"text": NARRATION_TEXT, "profile": profile_id})
    narration_id = start.json()["narration_id"]

    async with client.stream("GET", f"/speak/{narration_id}/stream") as response:
        assert response.status_code == 200
        # A second consumer must not double-synthesize the same narration.
        second = await client.get(f"/speak/{narration_id}/stream")
        assert second.status_code == 409

        stub_backend.gate.set()
        async for _event, _payload in iter_events(response):
            pass


async def test_unknown_narration_is_404(client, stub_backend):
    response = await client.get("/speak/not-a-real-narration/stream")
    assert response.status_code == 404
