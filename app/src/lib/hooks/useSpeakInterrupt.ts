import { listen } from '@tauri-apps/api/event';
import { useEffect } from 'react';
import { usePlatform } from '@/platform/PlatformContext';
import { usePlayerStore } from '@/stores/playerStore';

/**
 * Stop in-app playback when the floating pill reports that the user started
 * talking.
 *
 * The pill runs in its own Tauri webview, so it cannot pause the main window's
 * WaveSurfer player directly — it emits ``speak:interrupt`` and this hook
 * reacts. `AudioPlayer` already pauses when `isPlaying` flips to false, so
 * setting the flag is enough.
 */
export function useSpeakInterrupt() {
  const platform = usePlatform();
  const isTauri = platform.metadata.isTauri;

  useEffect(() => {
    if (!isTauri) return;

    const unlisten = listen('speak:interrupt', () => {
      usePlayerStore.getState().setIsPlaying(false);
    });

    return () => {
      unlisten.then((fn) => fn()).catch(() => {});
    };
  }, [isTauri]);
}
