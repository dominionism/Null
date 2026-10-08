"""Tests for the chunked streaming synthesis primitive.

The narration lane streams one chunk at a time so playback can start before the
whole utterance is synthesized. These tests pin the chunking contract and prove
that single-sourcing ``generate_chunked`` onto the streaming generator did not
change its output.
"""

import numpy as np
import pytest

from backend.utils.chunked_tts import (
    DEFAULT_MAX_CHUNK_CHARS,
    NARRATION_CHUNK_PAUSE_MS,
    apply_edge_fades,
    concatenate_audio_chunks,
    generate_chunked,
    generate_chunked_stream,
    split_text_into_chunks,
)

SAMPLE_RATE = 24000
SAMPLES_PER_CHAR = 40
LONG_TEXT = (
    "The first sentence explains what the agent is doing right now. "
    "The second sentence explains why it is doing that. "
    "The third sentence reports what happened when it tried. "
    "The fourth sentence says what it will try next."
)


class StubBackend:
    """Deterministic backend: audio depends only on (text, seed)."""

    def __init__(self) -> None:
        self.calls: list[tuple[str, int | None]] = []

    async def generate(
        self,
        text: str,
        voice_prompt: dict,
        language: str = "en",
        seed: int | None = None,
        instruct: str | None = None,
    ) -> tuple[np.ndarray, int]:
        self.calls.append((text, seed))
        return expected_chunk_audio(text, seed), SAMPLE_RATE


def expected_chunk_audio(text: str, seed: int | None) -> np.ndarray:
    """Independent reference for what a stub backend returns for one chunk."""
    samples = max(1, len(text) * SAMPLES_PER_CHAR)
    scale = float((seed or 0) % 7) + 1.0
    ramp = np.arange(samples, dtype=np.float32)
    return (0.4 * np.sin(ramp * scale / 13.0)).astype(np.float32)


def reference_generate_chunked(
    text: str,
    seed: int | None,
    max_chunk_chars: int = DEFAULT_MAX_CHUNK_CHARS,
    crossfade_ms: int = 50,
) -> np.ndarray:
    """The pre-refactor algorithm, reimplemented independently."""
    chunks = split_text_into_chunks(text, max_chunk_chars)
    if len(chunks) <= 1:
        return expected_chunk_audio(text, seed)
    audios = [
        expected_chunk_audio(chunk, (seed + i) if seed is not None else None)
        for i, chunk in enumerate(chunks)
    ]
    return concatenate_audio_chunks(audios, SAMPLE_RATE, crossfade_ms=crossfade_ms)


async def test_generate_chunked_matches_pre_refactor_reference():
    backend = StubBackend()
    audio, sample_rate = await generate_chunked(
        backend,
        LONG_TEXT,
        {},
        seed=7,
        max_chunk_chars=160,
        crossfade_ms=50,
    )
    assert sample_rate == SAMPLE_RATE
    assert np.array_equal(audio, reference_generate_chunked(LONG_TEXT, 7, 160, 50))


async def test_generate_chunked_hard_cut_matches_reference():
    backend = StubBackend()
    audio, _ = await generate_chunked(
        backend,
        LONG_TEXT,
        {},
        seed=11,
        max_chunk_chars=160,
        crossfade_ms=0,
    )
    assert np.array_equal(audio, reference_generate_chunked(LONG_TEXT, 11, 160, 0))


async def test_generate_chunked_single_chunk_fast_path_unchanged():
    text = "Short line."
    backend = StubBackend()
    audio, _ = await generate_chunked(backend, text, {}, seed=3)
    # The fast path forwards the ORIGINAL text with the base seed (no +i).
    assert backend.calls == [(text, 3)]
    assert np.array_equal(audio, expected_chunk_audio(text, 3))


async def test_stream_yields_chunks_in_text_order_with_varied_seeds():
    backend = StubBackend()
    chunk_texts = []
    async for _audio, sample_rate, chunk_text in generate_chunked_stream(
        backend,
        LONG_TEXT,
        {},
        seed=1,
        max_chunk_chars=160,
        fade_edges=False,
    ):
        assert sample_rate == SAMPLE_RATE
        chunk_texts.append(chunk_text)

    expected_chunks = split_text_into_chunks(LONG_TEXT, 160)
    assert chunk_texts == expected_chunks
    assert [text for text, _seed in backend.calls] == expected_chunks
    assert [seed for _text, seed in backend.calls] == [1 + i for i in range(len(expected_chunks))]


async def test_stream_chunks_concatenate_to_generate_chunked():
    backend = StubBackend()
    parts = []
    async for audio, _sample_rate, _chunk_text in generate_chunked_stream(
        backend,
        LONG_TEXT,
        {},
        seed=5,
        max_chunk_chars=160,
        crossfade_ms=50,
        fade_edges=False,
    ):
        parts.append(audio)

    assert len(parts) == len(split_text_into_chunks(LONG_TEXT, 160))
    combined = concatenate_audio_chunks(parts, SAMPLE_RATE, crossfade_ms=50)
    assert np.array_equal(combined, reference_generate_chunked(LONG_TEXT, 5, 160, 50))


async def test_stream_edges_are_faded_for_butt_joining():
    backend = StubBackend()
    fade_ms = 50
    requested = int(SAMPLE_RATE * fade_ms / 1000)
    seen = 0
    async for audio, _sample_rate, _chunk_text in generate_chunked_stream(
        backend,
        LONG_TEXT,
        {},
        max_chunk_chars=160,
        crossfade_ms=fade_ms,
    ):
        seen += 1
        fade = min(requested, len(audio) // 2)
        assert fade > 0
        assert audio[0] == pytest.approx(0.0, abs=1e-6)
        assert audio[-1] == pytest.approx(0.0, abs=1e-6)
        core = audio[fade : len(audio) - fade]
        if core.size:
            assert np.max(np.abs(core)) > 0.1

    assert seen == len(split_text_into_chunks(LONG_TEXT, 160))


async def test_trim_runs_once_per_chunk():
    backend = StubBackend()
    trimmed: list[int] = []

    def trim(audio: np.ndarray, sample_rate: int) -> np.ndarray:
        trimmed.append(len(audio))
        return audio * np.float32(0.5)

    chunk_count = 0
    async for audio, _sample_rate, _chunk_text in generate_chunked_stream(
        backend,
        LONG_TEXT,
        {},
        max_chunk_chars=160,
        trim_fn=trim,
        fade_edges=False,
    ):
        chunk_count += 1
        assert np.max(np.abs(audio)) <= 0.2 + 1e-6

    assert len(trimmed) == chunk_count == len(split_text_into_chunks(LONG_TEXT, 160))


class RunawayOnce:
    """Flags the first chunk it sees as unstable, then passes everything."""

    def __init__(self) -> None:
        self.calls = 0

    def __call__(self, audio: np.ndarray, sample_rate: int) -> bool:
        self.calls += 1
        return self.calls == 1


async def test_runaway_retry_splits_and_retries_smaller_chunks():
    text = " ".join(f"Sentence number {i} reports the current state." for i in range(4))
    assert len(text) > 100, "retry path needs text longer than MIN_RUNAWAY_RETRY_CHARS"

    backend = StubBackend()
    detector = RunawayOnce()
    audio, sample_rate = await generate_chunked(
        backend,
        text,
        {},
        seed=2,
        max_chunk_chars=500,
        crossfade_ms=0,
        runaway_detector=detector,
    )

    assert sample_rate == SAMPLE_RATE
    assert backend.calls[0] == (text, 2)
    assert len(backend.calls) > 2, "expected the unstable chunk to be retried in smaller pieces"
    # Retry seeds are chunk_seed + ((retry_depth + 1) * 1000) + i with depth 0.
    assert [seed for _text, seed in backend.calls[1:]] == [
        2 + 1000 + i for i in range(len(backend.calls) - 1)
    ]
    expected = np.concatenate([expected_chunk_audio(text, seed) for text, seed in backend.calls[1:]])
    assert np.array_equal(audio, expected)


def test_apply_edge_fades_caps_fade_at_half_length():
    audio = np.ones(10, dtype=np.float32)
    faded = apply_edge_fades(audio, SAMPLE_RATE, 1000)
    assert len(faded) == 10
    assert faded[0] == pytest.approx(0.0, abs=1e-6)
    assert faded[-1] == pytest.approx(0.0, abs=1e-6)
    assert faded[5] == pytest.approx(1.0, abs=1e-6)


def test_apply_edge_fades_noop_when_disabled():
    audio = np.ones(500, dtype=np.float32)
    assert np.array_equal(apply_edge_fades(audio, SAMPLE_RATE, 0), audio)


async def test_stream_keeps_a_short_utterance_in_one_chunk():
    # A multi-sentence line short enough to fit the budget must reach the engine
    # as ONE call, so the model keeps its cross-sentence prosody. Splitting it
    # per sentence is what made narration sound like separate reads.
    text = "First sentence here. Second sentence here. Third sentence here."
    backend = StubBackend()
    seen = [
        chunk_text
        async for _audio, _sample_rate, chunk_text in generate_chunked_stream(backend, text, {})
    ]
    assert seen == [text]
    assert [t for t, _seed in backend.calls] == [text]


async def test_stream_pauses_between_chunks():
    # Chunks are joined by a real pause, not a fade to zero: the boundary should
    # sound like a breath between sentences rather than a dropout.
    backend = StubBackend()
    chunks = [
        (audio, chunk_text)
        async for audio, _sample_rate, chunk_text in generate_chunked_stream(
            backend, LONG_TEXT, {}, max_chunk_chars=160
        )
    ]
    assert len(chunks) > 1
    pause = int(SAMPLE_RATE * NARRATION_CHUNK_PAUSE_MS / 1000)
    for audio, _text in chunks[:-1]:
        assert np.all(np.abs(audio[-pause:]) < 1e-6)
        assert np.any(np.abs(audio[:-pause]) > 0.01)
    last, _text = chunks[-1]
    assert np.any(np.abs(last[-pause:]) > 0.01)
