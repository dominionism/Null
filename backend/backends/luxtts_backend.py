"""
LuxTTS backend implementation.

Wraps the LuxTTS (ZipVoice) model for zero-shot voice cloning.
~1GB VRAM, 48kHz output, 150x realtime on CPU.
"""

import asyncio
import logging
from typing import Optional, Tuple

import numpy as np

from . import TTSBackend
from .base import (
    is_model_cached,
    get_torch_device,
    empty_device_cache,
    manual_seed,
    combine_voice_prompts as _combine_voice_prompts,
    model_load_progress,
)
from ..utils.cache import get_cache_key, get_cached_voice_prompt, cache_voice_prompt

logger = logging.getLogger(__name__)

# HuggingFace repo for model weight detection
LUXTTS_HF_REPO = "YatharthS/LuxTTS"

# LuxTTS multiplies the requested speed by 1.3 internally ("default is too slow"),
# so the 1.0 we used to pass spoke at 206 wpm — a compressed, run-on delivery
# against a natural 140-160 wpm. 0.75 lands at ~155 wpm with the sentence pauses
# intact (measured on this machine: 1.0 -> 206 wpm, 0.8 -> 164, 0.7 -> 144).
LUXTTS_SPEED = 0.75

# Diffusion steps. This is the user waiting to hear the agent, so it is set at the
# knee of the curve rather than for maximum detail — measured on this machine, same
# text and seed, same voice (synthesis seconds, audio produced, level):
#   4 steps  1.68 s  4.73 s  -25.9 dBFS
#   6 steps  1.81 s  4.73 s  -25.3 dBFS
#   8 steps  2.38 s  4.73 s  -24.9 dBFS
#  12 steps  3.51 s  4.73 s  -24.6 dBFS
# Pacing is identical at every setting and the level differs by ~1 dB, so 6 buys
# 0.6 s of the back half for the smallest audible difference; 8 -> 12 costs 1.1 s
# for +0.3 dB, which is past the knee.
LUXTTS_STEPS = 6


class LuxTTSBackend:
    """LuxTTS backend for zero-shot voice cloning."""

    def __init__(self):
        self.model = None
        self.model_size = "default"  # LuxTTS has only one model size
        self._device = None

    def _get_device(self) -> str:
        return get_torch_device(allow_mps=True, allow_xpu=True)

    def is_loaded(self) -> bool:
        return self.model is not None

    @property
    def device(self) -> str:
        if self._device is None:
            self._device = self._get_device()
        return self._device

    def _get_model_path(self, model_size: str) -> str:
        return LUXTTS_HF_REPO

    def _is_model_cached(self, model_size: str = "default") -> bool:
        return is_model_cached(
            LUXTTS_HF_REPO,
            weight_extensions=(".pt", ".safetensors", ".onnx", ".bin"),
        )

    async def load_model(self, model_size: str = "default") -> None:
        """Load the LuxTTS model."""
        if self.model is not None:
            return

        await asyncio.to_thread(self._load_model_sync)

    def _load_model_sync(self):
        model_name = "luxtts"
        is_cached = self._is_model_cached()

        with model_load_progress(model_name, is_cached):
            from zipvoice.luxvoice import LuxTTS

            device = self.device
            logger.info(f"Loading LuxTTS on {device}...")

            if device == "cpu":
                import os

                threads = os.cpu_count() or 4
                self.model = LuxTTS(
                    model_path=LUXTTS_HF_REPO,
                    device="cpu",
                    threads=min(threads, 8),
                )
            else:
                self.model = LuxTTS(model_path=LUXTTS_HF_REPO, device=device)

        logger.info("LuxTTS loaded successfully")

    def unload_model(self) -> None:
        """Unload model to free memory."""
        if self.model is not None:
            device = self.device
            del self.model
            self.model = None
            self._device = None

            empty_device_cache(device)

            logger.info("LuxTTS unloaded")

    async def create_voice_prompt(
        self,
        audio_path: str,
        reference_text: str,
        use_cache: bool = True,
    ) -> Tuple[dict, bool]:
        """
        Create voice prompt from reference audio.

        LuxTTS uses its own encode_prompt() which runs Whisper ASR internally
        to transcribe the reference. The reference_text parameter is not used
        by LuxTTS itself, but we include it in the cache key for consistency.
        """
        await self.load_model()

        # Compute cache key once for both lookup and storage
        cache_key = ("luxtts_" + get_cache_key(audio_path, reference_text)) if use_cache else None

        if cache_key:
            cached = get_cached_voice_prompt(cache_key)
            if cached is not None and isinstance(cached, dict):
                return cached, True

        def _encode_sync():
            return self.model.encode_prompt(
                prompt_audio=str(audio_path),
                duration=5,
                rms=0.01,
            )

        encoded = await asyncio.to_thread(_encode_sync)

        if cache_key:
            cache_voice_prompt(cache_key, encoded)

        return encoded, False

    async def combine_voice_prompts(self, audio_paths, reference_texts):
        return await _combine_voice_prompts(audio_paths, reference_texts, sample_rate=24000)

    async def generate(
        self,
        text: str,
        voice_prompt: dict,
        language: str = "en",
        seed: Optional[int] = None,
        instruct: Optional[str] = None,
    ) -> Tuple[np.ndarray, int]:
        """
        Generate audio from text using LuxTTS.

        Args:
            text: Text to synthesize
            voice_prompt: Encoded prompt dict from encode_prompt()
            language: Language code (LuxTTS is English-focused)
            seed: Random seed for reproducibility
            instruct: Not supported by LuxTTS (ignored)

        Returns:
            Tuple of (audio_array, sample_rate)
        """
        await self.load_model()

        def _generate_sync():
            if seed is not None:
                manual_seed(seed, self.device)

            wav = self.model.generate_speech(
                text=text,
                encode_dict=voice_prompt,
                num_steps=LUXTTS_STEPS,
                guidance_scale=3.0,
                t_shift=0.5,
                speed=LUXTTS_SPEED,
                return_smooth=False,  # 48kHz output
            )

            # LuxTTS returns a tensor (may be on GPU/MPS), move to CPU first
            audio = wav.detach().cpu().numpy().squeeze()
            return audio, 48000

        return await asyncio.to_thread(_generate_sync)
