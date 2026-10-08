/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
/**
 * Response for ``POST /speak/narrate`` and ``voicebox.speak(narrate=True)``.
 *
 * ``mode`` says which path was taken:
 *
 * * ``"narration"`` — CPU narration lane; the caller streams the audio from
 * ``stream_url``. Ephemeral: no generations row, no history entry.
 * * ``"generation"`` — the profile has no CPU narration voice, so the request
 * degraded to the normal queued speak path; poll ``poll_url``.
 */
export type NarrateSpeakResponse = {
    mode: string;
    narration_id?: (string | null);
    stream_url?: (string | null);
    generation_id?: (string | null);
    poll_url?: (string | null);
    status?: (string | null);
    profile: string;
    engine?: (string | null);
};

