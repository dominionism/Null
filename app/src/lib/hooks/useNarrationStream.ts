import { useCallback, useEffect, useRef } from 'react';
import { apiClient } from '@/lib/api/client';
import { debug } from '@/lib/utils/debug';
import {
  beginNarration,
  onNarrationDrained,
  scheduleNarrationChunk,
} from '@/lib/utils/narrationPlayer';
import { mark } from '@/lib/utils/turnTrace';

interface NarrationChunkEvent {
  index: number;
  sample_rate: number;
  text: string;
  wav: string;
}

interface NarrationStreamHandlers {
  /** Fired once, when the first chunk actually starts playing. */
  onStart?: () => void;
  /** Fired on every stream event, so the caller can reset its watchdog. */
  onActivity?: () => void;
  /** Fired when the stream fails; the narration will not continue. */
  onFailure?: (reason: string) => void;
  /** Fired when the stream finished AND the last buffered chunk stopped playing. */
  onEnd?: () => void;
}

/**
 * Consume a narration session's SSE stream.
 *
 * The stream is one-shot and not resumable: it synthesizes on demand, so an
 * EventSource reconnect would re-synthesize the whole narration. Every failure
 * path therefore closes the source instead of letting it retry.
 *
 * Starting a stream cancels the previous one ("last speak wins").
 */
export function useNarrationStream() {
  const sourceRef = useRef<EventSource | null>(null);

  const stop = useCallback(() => {
    sourceRef.current?.close();
    sourceRef.current = null;
    // Bumping the epoch drops any chunk that is mid-decode, so stopping is
    // immediate rather than "as soon as the pending decode finishes".
    beginNarration();
  }, []);

  const start = useCallback((narrationId: string, handlers: NarrationStreamHandlers = {}) => {
    sourceRef.current?.close();
    sourceRef.current = null;
    const streamEpoch = beginNarration();
    // Dev trace: only the first chunk counts as first_chunk for this stream.
    let sawFirstChunk = false;

    const finish = (reason: string) => {
      sourceRef.current?.close();
      sourceRef.current = null;
      if (reason) {
        debug.warn('[narration] stream ended:', reason);
        handlers.onFailure?.(reason);
      }
    };

    mark(narrationId, 'stream_open');
    const source = new EventSource(apiClient.getNarrationStreamUrl(narrationId));
    sourceRef.current = source;

    source.addEventListener('ready', () => handlers.onActivity?.());
    source.addEventListener('loading', () => handlers.onActivity?.());
    // The server waits for the generation queue to drain before it synthesizes;
    // these events keep the pill's inactivity watchdog alive meanwhile.
    source.addEventListener('waiting', () => handlers.onActivity?.());

    source.addEventListener('chunk', (event) => {
      handlers.onActivity?.();
      if (!sawFirstChunk) {
        sawFirstChunk = true;
        mark(narrationId, 'first_chunk');
      }
      try {
        const data = JSON.parse((event as MessageEvent<string>).data) as NarrationChunkEvent;
        void scheduleNarrationChunk(streamEpoch, data.wav, handlers.onStart);
      } catch (err) {
        debug.error('[narration] malformed chunk event:', err);
      }
    });

    source.addEventListener('done', () => {
      mark(narrationId, 'last_chunk');
      // The stream is finished, but buffered chunks may still be playing —
      // wait for the tail before telling the caller the narration is over.
      onNarrationDrained(streamEpoch, () => {
        finish('');
        handlers.onEnd?.();
      });
    });

    // NOTE: 'error' is also EventSource's transport-error event name, so this
    // listener fires for both a server-sent `error` event and a dropped
    // connection. Both must terminate the stream — never reconnect.
    source.addEventListener('error', (event) => {
      const data = (event as MessageEvent<string>).data;
      if (typeof data === 'string' && data.length > 0) {
        try {
          const parsed = JSON.parse(data) as { message?: string };
          finish(parsed.message ?? 'narration failed');
          return;
        } catch {
          // Fall through to the generic transport failure below.
        }
      }
      finish('narration stream closed');
    });
  }, []);

  useEffect(() => stop, [stop]);

  return { start, stop };
}
