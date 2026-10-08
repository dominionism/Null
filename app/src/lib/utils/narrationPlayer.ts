import { debug } from '@/lib/utils/debug';

/**
 * Gapless player for streamed agent narration.
 *
 * The backend synthesizes one sentence-sized chunk at a time and sends each as
 * a base64 WAV. Chunks are decoded and scheduled against a single AudioContext
 * clock so they butt-join without a seam, which is why this does not use one
 * HTMLAudioElement per chunk.
 *
 * Tauri/wry enables webview autoplay by default, so the context runs without a
 * user gesture in the pill window.
 *
 * Cancellation is epoch-based: `beginNarration` starts a fresh generation of
 * state, and any chunk that resolves afterwards belongs to the discarded state
 * and is dropped. That is what makes "last speak wins" and barge-in
 * instantaneous instead of waiting on the in-flight decode.
 */

/** Scheduling lead, in seconds — absorbs decode jitter without a gap. */
const START_LEAD_SECONDS = 0.15;

interface NarrationState {
  epoch: number;
  /** Chunks handed to the decoder but not yet scheduled. */
  pendingDecodes: number;
  sources: AudioBufferSourceNode[];
  nextStartAt: number;
  /** Someone is waiting for the tail of this narration to finish playing. */
  drainRequested: boolean;
  drainCallbacks: Array<() => void>;
}

let ctx: AudioContext | null = null;
let state: NarrationState = {
  epoch: 0,
  pendingDecodes: 0,
  sources: [],
  nextStartAt: 0,
  drainRequested: false,
  drainCallbacks: [],
};

function getContext(): AudioContext {
  if (!ctx) {
    ctx = new AudioContext();
  }
  if (ctx.state === 'suspended') {
    void ctx.resume().catch((err) => debug.warn('[narration] resume failed:', err));
  }
  return ctx;
}

/**
 * Fire pending drain callbacks once this narration has nothing left to play.
 *
 * Waiting only on scheduled sources is not enough: the stream can end while
 * chunks are still decoding, and treating that as "finished" would cut the
 * narration off before it is heard.
 */
function maybeDrain(target: NarrationState): void {
  if (!target.drainRequested || target.pendingDecodes > 0 || target.sources.length > 0) {
    return;
  }

  target.drainRequested = false;
  const callbacks = target.drainCallbacks;
  target.drainCallbacks = [];
  for (const callback of callbacks) callback();
}

/**
 * Start a new narration, cancelling any narration already playing or
 * scheduled. Returns the epoch that `scheduleNarrationChunk` must be given so
 * chunks belonging to a superseded narration are dropped rather than played.
 *
 * Stopping is the same operation as starting: there is no "stop" that leaves
 * the player ready to resume a cancelled utterance, because barge-in and
 * last-speak-wins are both terminal for the interrupted narration.
 */
export function beginNarration(): number {
  for (const source of state.sources) {
    try {
      source.onended = null;
      source.stop();
      source.disconnect();
    } catch {
      // Already stopped or never started — nothing to clean up.
    }
  }

  state = {
    epoch: state.epoch + 1,
    pendingDecodes: 0,
    sources: [],
    nextStartAt: 0,
    drainRequested: false,
    drainCallbacks: [],
  };
  return state.epoch;
}

function base64ToArrayBuffer(base64: string): ArrayBuffer {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes.buffer;
}

/**
 * Decode one narration chunk and schedule it to butt-join the previous one.
 *
 * @param chunkEpoch epoch returned by `beginNarration` for this narration
 * @param wavBase64 base64-encoded WAV bytes from a `chunk` SSE event
 * @param onStart called once, when the first chunk of this narration is scheduled
 */
export async function scheduleNarrationChunk(
  chunkEpoch: number,
  wavBase64: string,
  onStart?: () => void,
): Promise<void> {
  const audioContext = getContext();
  const target = state;
  if (chunkEpoch !== target.epoch) return;

  target.pendingDecodes += 1;

  let buffer: AudioBuffer;
  try {
    buffer = await audioContext.decodeAudioData(base64ToArrayBuffer(wavBase64));
  } catch (err) {
    debug.error('[narration] decode failed:', err);
    target.pendingDecodes -= 1;
    maybeDrain(target);
    return;
  }

  // The narration may have been replaced or cancelled while this decoded.
  if (chunkEpoch !== target.epoch) return;

  const isFirst = target.sources.length === 0;
  const source = audioContext.createBufferSource();
  source.buffer = buffer;
  source.connect(audioContext.destination);

  const startAt = Math.max(audioContext.currentTime + START_LEAD_SECONDS, target.nextStartAt);
  source.start(startAt);
  target.sources.push(source);
  target.nextStartAt = startAt + buffer.duration;
  target.pendingDecodes -= 1;

  // Drop the reference once played, so the array cannot grow forever, and let
  // a pending drain know when the last chunk has actually finished.
  source.onended = () => {
    target.sources = target.sources.filter((s) => s !== source);
    maybeDrain(target);
  };

  if (isFirst) {
    onStart?.();
  }
}

/**
 * Invoke ``onDrained`` once every chunk of this narration has finished playing.
 *
 * The SSE stream ends as soon as the last chunk is *sent*, which can be well
 * before the buffered audio finishes — the pill must stay visible for the
 * whole utterance, so callers use this instead of dismissing on `done`.
 * Fires immediately when the narration was cancelled or has nothing to play.
 */
export function onNarrationDrained(streamEpoch: number, onDrained: () => void): void {
  if (streamEpoch !== state.epoch) {
    onDrained();
    return;
  }

  state.drainRequested = true;
  state.drainCallbacks.push(onDrained);
  maybeDrain(state);
}
