"""
Chunked TTS generation utilities.

Splits long text into sentence-boundary chunks, generates audio per-chunk
via any TTSBackend, and concatenates with crossfade.  All logic is
engine-agnostic — it wraps the standard ``TTSBackend.generate()`` interface.

Short text (≤ max_chunk_chars) uses the single-shot fast path with zero
overhead.
"""

import logging
import re
from collections.abc import AsyncIterator

import numpy as np

logger = logging.getLogger("voicebox.chunked-tts")

# Default chunk size in characters.  Can be overridden per-request via
# the ``max_chunk_chars`` field on GenerationRequest.
DEFAULT_MAX_CHUNK_CHARS = 800

# Narration joins its chunks with a natural pause rather than a crossfade. The
# fade only suppresses clicks; the silence is what makes a sentence boundary
# sound like speech instead of a dropout. (Splitting one sentence per chunk and
# fading each to zero made a multi-sentence line sound like separate reads.)
NARRATION_EDGE_FADE_MS = 10
NARRATION_CHUNK_PAUSE_MS = 220
MAX_RUNAWAY_RETRIES = 2
MIN_RUNAWAY_RETRY_CHARS = 100

# Common abbreviations that should NOT be treated as sentence endings.
# Lowercase for case-insensitive matching.
_ABBREVIATIONS = frozenset(
    {
        "mr",
        "mrs",
        "ms",
        "dr",
        "prof",
        "sr",
        "jr",
        "st",
        "ave",
        "blvd",
        "inc",
        "ltd",
        "corp",
        "dept",
        "est",
        "approx",
        "vs",
        "etc",
        "e.g",
        "i.e",
        "a.m",
        "p.m",
        "u.s",
        "u.s.a",
        "u.k",
    }
)

# Paralinguistic tags used by Chatterbox Turbo.  The splitter must never
# cut inside one of these.
_PARA_TAG_RE = re.compile(r"\[[^\]]*\]")


def split_text_into_chunks(text: str, max_chars: int = DEFAULT_MAX_CHUNK_CHARS) -> list[str]:
    """Split *text* at natural boundaries into chunks of at most *max_chars*.

    Priority: sentence-end (``.!?`` not preceded by an abbreviation and not
    inside brackets) → clause boundary (``;:,—``) → whitespace → hard cut.

    Paralinguistic tags like ``[laugh]`` are treated as atomic and will not
    be split across chunks.
    """
    text = text.strip()
    if not text:
        return []
    if len(text) <= max_chars:
        return [text]

    chunks: list[str] = []
    remaining = text

    while remaining:
        remaining = remaining.lstrip()
        if not remaining:
            break
        if len(remaining) <= max_chars:
            chunks.append(remaining)
            break

        segment = remaining[:max_chars]

        # Try to split at the last real sentence ending
        split_pos = _find_last_sentence_end(segment)
        if split_pos == -1:
            split_pos = _find_last_clause_boundary(segment)
        if split_pos == -1:
            split_pos = segment.rfind(" ")
        if split_pos == -1:
            # Absolute fallback: hard cut but avoid splitting inside a tag
            split_pos = _safe_hard_cut(segment, max_chars)

        chunk = remaining[: split_pos + 1].strip()
        if chunk:
            chunks.append(chunk)
        remaining = remaining[split_pos + 1 :]

    return chunks


def _iter_sentence_ends(text: str):
    """Yield the indices of real sentence-ending punctuation in *text*.

    Skips periods that follow common abbreviations (``Dr.``, ``Mr.``, etc.)
    and periods inside bracket tags (``[laugh]``).  Also yields CJK
    sentence-ending punctuation (``。！？``).
    """
    for m in re.finditer(r"[.!?](?:\s|$)", text):
        pos = m.start()
        char = text[pos]
        # Skip periods after abbreviations
        if char == ".":
            # Walk backwards to find the preceding word
            word_start = pos - 1
            while word_start >= 0 and text[word_start].isalpha():
                word_start -= 1
            word = text[word_start + 1 : pos].lower()
            if word in _ABBREVIATIONS:
                continue
            # Skip decimal numbers (digit immediately before the period)
            if word_start >= 0 and text[word_start].isdigit():
                continue
        # Skip if we're inside a bracket tag
        if _inside_bracket_tag(text, pos):
            continue
        yield pos
    for m in re.finditer(r"[\u3002\uff01\uff1f]", text):
        yield m.start()


def _find_last_sentence_end(text: str) -> int:
    """Return the index of the last sentence-ending punctuation in *text*."""
    return max(_iter_sentence_ends(text), default=-1)


def _find_last_clause_boundary(text: str) -> int:
    """Return the index of the last clause-boundary punctuation."""
    best = -1
    for m in re.finditer(r"[;:,\u2014](?:\s|$)", text):
        pos = m.start()
        # Skip if inside a bracket tag
        if _inside_bracket_tag(text, pos):
            continue
        best = pos
    return best


def _inside_bracket_tag(text: str, pos: int) -> bool:
    """Return True if *pos* falls inside a ``[...]`` tag."""
    for m in _PARA_TAG_RE.finditer(text):
        if m.start() < pos < m.end():
            return True
    return False


def _safe_hard_cut(segment: str, max_chars: int) -> int:
    """Find a hard-cut position that doesn't split a ``[tag]``."""
    cut = max_chars - 1
    # Check if the cut falls inside a bracket tag; if so, move before it
    for m in _PARA_TAG_RE.finditer(segment):
        if m.start() < cut < m.end():
            return m.start() - 1 if m.start() > 0 else cut
    return cut


def concatenate_audio_chunks(
    chunks: list[np.ndarray],
    sample_rate: int,
    crossfade_ms: int = 50,
) -> np.ndarray:
    """Concatenate audio arrays with a short crossfade to eliminate clicks.

    Each chunk is expected to be a 1-D float32 ndarray at *sample_rate* Hz.
    """
    if not chunks:
        return np.array([], dtype=np.float32)
    if len(chunks) == 1:
        return chunks[0]

    crossfade_samples = int(sample_rate * crossfade_ms / 1000)
    result = np.array(chunks[0], dtype=np.float32, copy=True)

    for chunk in chunks[1:]:
        if len(chunk) == 0:
            continue
        overlap = min(crossfade_samples, len(result), len(chunk))
        if overlap > 0:
            fade_out = np.linspace(1.0, 0.0, overlap, dtype=np.float32)
            fade_in = np.linspace(0.0, 1.0, overlap, dtype=np.float32)
            result[-overlap:] = result[-overlap:] * fade_out + chunk[:overlap] * fade_in
            result = np.concatenate([result, chunk[overlap:]])
        else:
            result = np.concatenate([result, chunk])

    return result


async def _generate_one(
    backend,
    chunk_text: str,
    chunk_seed: int | None,
    *,
    voice_prompt: dict,
    language: str,
    instruct: str | None,
    trim_fn,
    runaway_detector,
    crossfade_ms: int,
    retry_depth: int = 0,
) -> tuple[np.ndarray, int]:
    """Generate a single chunk, retrying in smaller pieces if output is unstable."""
    chunk_audio, chunk_sr = await backend.generate(
        chunk_text,
        voice_prompt,
        language,
        chunk_seed,
        instruct,
    )

    if runaway_detector is not None and runaway_detector(chunk_audio, chunk_sr):
        if retry_depth >= MAX_RUNAWAY_RETRIES or len(chunk_text) <= MIN_RUNAWAY_RETRY_CHARS:
            raise RuntimeError(
                "TTS output remained unstable after retrying smaller text chunks"
            )

        retry_max_chars = max(MIN_RUNAWAY_RETRY_CHARS, len(chunk_text) // 2)
        retry_chunks = split_text_into_chunks(chunk_text, retry_max_chars)
        if len(retry_chunks) <= 1:
            raise RuntimeError("Unable to split unstable TTS output for retry")

        logger.warning(
            "Detected unstable TTS output for %d chars; retrying as %d smaller chunks",
            len(chunk_text),
            len(retry_chunks),
        )
        retry_audio: list[np.ndarray] = []
        for i, retry_text in enumerate(retry_chunks):
            retry_seed = (
                chunk_seed + ((retry_depth + 1) * 1000) + i
                if chunk_seed is not None
                else None
            )
            audio, sample_rate = await _generate_one(
                backend,
                retry_text,
                retry_seed,
                voice_prompt=voice_prompt,
                language=language,
                instruct=instruct,
                trim_fn=trim_fn,
                runaway_detector=runaway_detector,
                crossfade_ms=crossfade_ms,
                retry_depth=retry_depth + 1,
            )
            retry_audio.append(np.asarray(audio, dtype=np.float32))

        return (
            concatenate_audio_chunks(
                retry_audio,
                sample_rate,
                crossfade_ms=crossfade_ms,
            ),
            sample_rate,
        )

    if trim_fn is not None:
        chunk_audio = trim_fn(chunk_audio, chunk_sr)
    return np.asarray(chunk_audio, dtype=np.float32), chunk_sr


def apply_edge_fades(audio: np.ndarray, sample_rate: int, fade_ms: int) -> np.ndarray:
    """
    Fade a chunk's head and tail so consecutive chunks can be butt-joined.

    ``concatenate_audio_chunks`` overlaps one chunk's tail with the next
    chunk's head, which is impossible to reproduce live (it would need the
    following chunk before the current one can be played). The streaming path
    instead fades each chunk's outer ``fade_ms`` and butts them together, so
    playback can start as soon as the first chunk exists.

    Uses the same linear windows as ``concatenate_audio_chunks``.
    """
    audio = np.asarray(audio, dtype=np.float32)
    fade_samples = min(int(sample_rate * fade_ms / 1000), len(audio) // 2)
    if fade_samples <= 0:
        return audio

    faded = audio.copy()
    faded[:fade_samples] *= np.linspace(0.0, 1.0, fade_samples, dtype=np.float32)
    faded[-fade_samples:] *= np.linspace(1.0, 0.0, fade_samples, dtype=np.float32)
    return faded


async def generate_chunked_stream(
    backend,
    text: str,
    voice_prompt: dict,
    language: str = "en",
    seed: int | None = None,
    instruct: str | None = None,
    max_chunk_chars: int = DEFAULT_MAX_CHUNK_CHARS,
    crossfade_ms: int = 50,
    trim_fn=None,
    runaway_detector=None,
    fade_edges: bool = True,
) -> AsyncIterator[tuple[np.ndarray, int, str]]:
    """
    Generate audio chunk by chunk, yielding each finished chunk immediately.

    Same splitting, per-chunk seed variation, trimming and runaway retry as
    :func:`generate_chunked`, but the caller receives ``(audio, sample_rate,
    chunk_text)`` per chunk instead of one concatenated buffer.

    ``fade_edges=False`` is used by :func:`generate_chunked` itself, so both
    paths share one generation loop; streamed audio is for live playback only and
    is never persisted.

    The yielded text is the chunk as produced by :func:`split_text_into_chunks`;
    a runaway retry that internally synthesized several sub-chunks is
    concatenated into that single yield.

    With ``fade_edges=True`` the chunks are shaped for narration: a short
    click-suppressing fade on each edge, plus a natural pause after every chunk
    but the last. Chunks are never rendered per sentence — an utterance short
    enough to fit *max_chunk_chars* is one chunk, so the engine keeps its
    cross-sentence prosody.
    """
    chunks = split_text_into_chunks(text, max_chunk_chars)

    if len(chunks) <= 1:
        # Short text — single-shot fast path. Passes the original text (not
        # chunks[0]) to stay identical to generate_chunked.
        audio, sample_rate = await _generate_one(
            backend,
            text,
            seed,
            voice_prompt=voice_prompt,
            language=language,
            instruct=instruct,
            trim_fn=trim_fn,
            runaway_detector=runaway_detector,
            crossfade_ms=crossfade_ms,
        )
        yield (
            apply_edge_fades(audio, sample_rate, NARRATION_EDGE_FADE_MS)
            if fade_edges
            else audio,
            sample_rate,
            text,
        )
        return

    logger.info(
        "Splitting %d chars into %d chunks (max %d chars each)",
        len(text),
        len(chunks),
        max_chunk_chars,
    )

    for i, chunk_text in enumerate(chunks):
        logger.info(
            "Generating chunk %d/%d (%d chars)",
            i + 1,
            len(chunks),
            len(chunk_text),
        )
        # Vary the seed per chunk to avoid correlated RNG artefacts,
        # but keep it deterministic so the same (text, seed) pair
        # always produces the same output.
        chunk_seed = (seed + i) if seed is not None else None

        audio, sample_rate = await _generate_one(
            backend,
            chunk_text,
            chunk_seed,
            voice_prompt=voice_prompt,
            language=language,
            instruct=instruct,
            trim_fn=trim_fn,
            runaway_detector=runaway_detector,
            crossfade_ms=crossfade_ms,
        )
        if fade_edges:
            audio = apply_edge_fades(audio, sample_rate, NARRATION_EDGE_FADE_MS)
            if i < len(chunks) - 1:
                # A sentence boundary should sound like a breath, not a cut.
                pause = np.zeros(
                    int(sample_rate * NARRATION_CHUNK_PAUSE_MS / 1000), dtype=np.float32
                )
                audio = np.concatenate([audio, pause])

        yield audio, sample_rate, chunk_text


async def generate_chunked(
    backend,
    text: str,
    voice_prompt: dict,
    language: str = "en",
    seed: int | None = None,
    instruct: str | None = None,
    max_chunk_chars: int = DEFAULT_MAX_CHUNK_CHARS,
    crossfade_ms: int = 50,
    trim_fn=None,
    runaway_detector=None,
) -> tuple[np.ndarray, int]:
    """Generate audio with automatic chunking for long text.

    For text shorter than *max_chunk_chars* this is a thin wrapper around
    ``backend.generate()`` with zero overhead.

    For longer text the input is split at natural sentence boundaries,
    each chunk is generated independently, optionally trimmed (useful for
    Chatterbox engines that hallucinate trailing noise), and the results
    are concatenated with a crossfade (or hard cut if *crossfade_ms* is 0).

    Parameters
    ----------
    backend : TTSBackend
        Any backend implementing the ``generate()`` protocol.
    text : str
        Input text (may be arbitrarily long).
    voice_prompt, language, seed, instruct
        Forwarded to ``backend.generate()`` verbatim.
    max_chunk_chars : int
        Maximum characters per chunk (default 800).
    crossfade_ms : int
        Crossfade duration in milliseconds between chunks.  0 for a hard
        cut with no overlap (default 50).
    trim_fn : callable | None
        Optional ``(audio, sample_rate) -> audio`` post-processing
        function applied to each chunk before concatenation (e.g.
        ``trim_tts_output`` for Chatterbox engines).
    runaway_detector : callable | None
        Optional ``(audio, sample_rate) -> bool`` detector. When it flags
        unstable output, the affected text is split in half and retried.

    Returns
    -------
    (audio, sample_rate) : Tuple[np.ndarray, int]
    """
    audio_chunks: list[np.ndarray] = []
    sample_rate: int | None = None

    async for chunk_audio, chunk_sr, _chunk_text in generate_chunked_stream(
        backend,
        text,
        voice_prompt,
        language=language,
        seed=seed,
        instruct=instruct,
        max_chunk_chars=max_chunk_chars,
        crossfade_ms=crossfade_ms,
        trim_fn=trim_fn,
        runaway_detector=runaway_detector,
        fade_edges=False,
    ):
        audio_chunks.append(chunk_audio)
        if sample_rate is None:
            sample_rate = chunk_sr

    audio = concatenate_audio_chunks(audio_chunks, sample_rate, crossfade_ms=crossfade_ms)
    return audio, sample_rate
