import { apiClient } from '@/lib/api/client';

/**
 * Dev instrumentation for the voice-loop latency work.
 *
 * Records coarse timing marks for a single voice-loop turn so the backend can
 * correlate upload → delivery → audio playback across the whole loop. This is
 * best-effort tracing only: every call is fire-and-forget, its errors are
 * swallowed, and nothing here may ever throw into or block the caller's path.
 * When tracing is disabled the backend still answers 200 and ignores the marks.
 */

/** Fire-and-forget POST; never awaited, never throws. */
function post(endpoint: string, body: unknown): void {
  void fetch(`${apiClient.getBaseUrl()}${endpoint}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  }).catch(() => {});
}

/**
 * Start a capture-side turn and return the id the frontend owns. The id then
 * travels to the backend on the capture / deliver requests.
 */
export function beginTurn(anchorEpochMs: number): string {
  const turnId = crypto.randomUUID();
  post(`/turns/${turnId}/begin`, { anchor_epoch_ms: anchorEpochMs });
  return turnId;
}

/** Record a stage mark for a turn. No-op when the id is missing. */
export function mark(
  turnId: string | null | undefined,
  stage: string,
  meta?: Record<string, unknown>,
): void {
  if (!turnId) return;
  post(`/turns/${turnId}/marks`, { stage, meta: meta ?? null });
}

/** Finish a turn (the `turn_end` mark). No-op when the id is missing. */
export function finishTurn(turnId: string | null | undefined): void {
  mark(turnId, 'turn_end');
}
