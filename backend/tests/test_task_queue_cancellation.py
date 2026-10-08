import asyncio

import pytest

from backend.services import task_queue


@pytest.mark.asyncio
async def test_cancel_queued_generation_skips_execution():
    task_queue.init_queue(force=True)

    running_started = asyncio.Event()
    release_running = asyncio.Event()
    queued_ran = asyncio.Event()

    async def running_job():
        running_started.set()
        await release_running.wait()

    async def queued_job():
        queued_ran.set()

    task_queue.enqueue_generation("gen-running", running_job())
    await asyncio.wait_for(running_started.wait(), timeout=1)

    task_queue.enqueue_generation("gen-queued", queued_job())
    assert task_queue.cancel_generation("gen-queued") == "queued"

    release_running.set()
    await asyncio.sleep(0.1)

    assert not queued_ran.is_set()


@pytest.mark.asyncio
async def test_cancel_running_generation_cancels_task():
    task_queue.init_queue(force=True)

    running_started = asyncio.Event()
    running_cancelled = asyncio.Event()

    async def running_job():
        running_started.set()
        try:
            await asyncio.Event().wait()
        except asyncio.CancelledError:
            running_cancelled.set()
            raise

    task_queue.enqueue_generation("gen-running", running_job())
    await asyncio.wait_for(running_started.wait(), timeout=1)

    assert task_queue.cancel_generation("gen-running") == "running"
    await asyncio.wait_for(running_cancelled.wait(), timeout=1)


async def test_wait_until_idle_waits_for_a_running_generation():
    """The narration lane gates on this before it shares the work engine."""
    task_queue.init_queue(force=True)

    release = asyncio.Event()
    started = asyncio.Event()

    async def running_job():
        started.set()
        await release.wait()

    task_queue.enqueue_generation("gen-busy", running_job())
    await asyncio.wait_for(started.wait(), timeout=1)

    # Busy: a short timeout must report the queue as not idle.
    assert await task_queue.wait_until_idle(0.3) is False

    release.set()
    assert await task_queue.wait_until_idle(2.0) is True
