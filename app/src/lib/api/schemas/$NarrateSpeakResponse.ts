/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
export const $NarrateSpeakResponse = {
    description: `Response for \`\`POST /speak/narrate\`\` and \`\`voicebox.speak(narrate=True)\`\`.
    \`\`mode\`\` says which path was taken:
     * \`\`"narration"\`\` — CPU narration lane; the caller streams the audio from
    \`\`stream_url\`\`. Ephemeral: no generations row, no history entry.
     * \`\`"generation"\`\` — the profile has no CPU narration voice, so the request
    degraded to the normal queued speak path; poll \`\`poll_url\`\`.`,
    properties: {
        mode: {
            type: 'string',
            isRequired: true,
        },
        narration_id: {
            type: 'any-of',
            contains: [{
                type: 'string',
            }, {
                type: 'null',
            }],
        },
        stream_url: {
            type: 'any-of',
            contains: [{
                type: 'string',
            }, {
                type: 'null',
            }],
        },
        generation_id: {
            type: 'any-of',
            contains: [{
                type: 'string',
            }, {
                type: 'null',
            }],
        },
        poll_url: {
            type: 'any-of',
            contains: [{
                type: 'string',
            }, {
                type: 'null',
            }],
        },
        status: {
            type: 'any-of',
            contains: [{
                type: 'string',
            }, {
                type: 'null',
            }],
        },
        profile: {
            type: 'string',
            isRequired: true,
        },
        engine: {
            type: 'any-of',
            contains: [{
                type: 'string',
            }, {
                type: 'null',
            }],
        },
    },
} as const;
