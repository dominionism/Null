"""Supervision for the dedicated narration worker process.

In-process narration shares the work engine with the generation queue, so it has
to wait for that queue to drain before it can synthesize. A separate process with
its own model instance removes the wait: two processes have independent MPS
contexts and synthesize concurrently (verified on this hardware), which two
threads sharing one model cannot — that aborts the process with
`_status < MTLCommandBufferStatusCommitted`.

The worker is the same application started with ``--role narration``, serving
only ``/health`` and the stateless ``POST /narration/synthesize``. It reads the
same SQLite file (profiles and samples) but never writes.
"""

from __future__ import annotations

import asyncio
import logging
import os
import socket
import subprocess
import sys
from pathlib import Path

import httpx

logger = logging.getLogger(__name__)

# The main server keeps its own port; the worker takes the next one so the pair
# is predictable in logs and in `lsof`.
PREFERRED_PORT = 17494
HEALTH_TIMEOUT_SECONDS = 120.0
HEALTH_POLL_SECONDS = 0.25
# The worker's warm-up loads a model; a first-time load of a GPU model is slow.
WARM_TIMEOUT_SECONDS = 600.0

_process: subprocess.Popen | None = None
_base_url: str | None = None


def worker_base_url() -> str | None:
    """Base URL of the running narration worker, or None when there is none."""
    return _base_url


def _is_voicebox_worker(port: int) -> bool:
    """Whether something already listening on *port* is one of our workers."""
    try:
        response = httpx.get(f"http://127.0.0.1:{port}/health", timeout=2.0)
        return response.status_code == 200 and "voicebox" in response.text.lower()
    except Exception:
        return False


def _pick_port() -> int:
    """Prefer the conventional worker port; fall back to an ephemeral one."""
    with socket.socket() as probe:
        try:
            probe.bind(("127.0.0.1", PREFERRED_PORT))
            return PREFERRED_PORT
        except OSError:
            probe.bind(("127.0.0.1", 0))
            return int(probe.getsockname()[1])


def _spawn_command(port: int, data_dir: Path) -> list[str]:
    args = ["--host", "127.0.0.1", "--port", str(port), "--data-dir", str(data_dir)]
    if getattr(sys, "frozen", False):
        # Packaged builds re-run the same executable; the packaged entry point
        # must accept --role (see backend/narration_main.py for the dev path).
        return [sys.executable, "--role", "narration", *args]
    return [sys.executable, "-m", "backend.narration_main", *args]


async def start_worker(data_dir: Path) -> str | None:
    """
    Start the narration worker (or adopt one already listening) and wait for it.

    Returns the worker's base URL, or None when it could not be started — the
    caller then keeps narration in-process.
    """
    global _process, _base_url

    if _base_url is not None:
        return _base_url

    if os.environ.get("VOICEBOX_NARRATION_WORKER", "1") in ("0", "false", "no"):
        logger.info("Narration worker disabled by VOICEBOX_NARRATION_WORKER")
        return None

    if _is_voicebox_worker(PREFERRED_PORT):
        _base_url = f"http://127.0.0.1:{PREFERRED_PORT}"
        logger.info("Narration worker: adopting the one already on port %d", PREFERRED_PORT)
        return _base_url

    port = _pick_port()
    try:
        _process = subprocess.Popen(
            _spawn_command(port, data_dir),
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            start_new_session=True,
        )
    except Exception:
        logger.exception("Narration worker failed to spawn; narration stays in-process")
        return None

    base_url = f"http://127.0.0.1:{port}"
    deadline = HEALTH_TIMEOUT_SECONDS
    waited = 0.0
    while waited < deadline:
        if _process.poll() is not None:
            logger.warning(
                "Narration worker exited during startup (code %s); narration stays in-process",
                _process.returncode,
            )
            _process = None
            return None
        try:
            response = httpx.get(f"{base_url}/health", timeout=2.0)
            if response.status_code == 200:
                _base_url = base_url
                logger.info("Narration worker ready on %s (pid %s)", base_url, _process.pid)
                if _should_warm():
                    from .task_queue import create_background_task

                    create_background_task(_warm_worker(base_url))
                return _base_url
        except Exception:
            pass
        await asyncio.sleep(HEALTH_POLL_SECONDS)
        waited += HEALTH_POLL_SECONDS

    logger.warning("Narration worker did not become healthy; narration stays in-process")
    stop_worker()
    return None


def _should_warm() -> bool:
    """Whether the user asked for startup model warm-up (capture settings)."""
    from ..database import get_db
    from .settings import get_capture_settings

    try:
        db = next(get_db())
        try:
            return bool(get_capture_settings(db).warm_models_on_startup)
        finally:
            db.close()
    except Exception:
        logger.exception("Could not read the warm-up setting; skipping the warm-up")
        return False


async def _warm_worker(base_url: str) -> None:
    """Ask the worker to load its narration model now instead of on the first line."""
    try:
        await asyncio.to_thread(httpx.post, f"{base_url}/narration/warm", timeout=WARM_TIMEOUT_SECONDS)
        logger.info("Narration worker warm-up requested")
    except Exception:
        logger.exception("Narration worker warm-up request failed; the first line loads in-line")


def stop_worker() -> None:
    """Terminate the worker we spawned (a graceful exit releases its model)."""
    global _process, _base_url

    if _process is not None and _process.poll() is None:
        _process.terminate()
        try:
            _process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            _process.kill()
        logger.info("Narration worker stopped")
    _process = None
    _base_url = None
