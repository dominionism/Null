/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
export const $CaptureSettingsResponse = {
    description: `Server-persisted defaults for the capture / refine flow.`,
    properties: {
        stt_model: {
            type: 'string',
            pattern: '^(base|small|medium|large|turbo)$',
        },
        agent_voice_enabled: {
            type: 'boolean',
        },
        agent_target: {
            type: 'any-of',
            contains: [{
                type: 'string',
            }, {
                type: 'null',
            }],
        },
        language: {
            type: 'string',
        },
        auto_refine: {
            type: 'boolean',
        },
        submit_after_paste: {
            type: 'boolean',
        },
        llm_model: {
            type: 'string',
            pattern: '^(0\\.6B|1\\.7B|4B)$',
        },
        smart_cleanup: {
            type: 'boolean',
        },
        self_correction: {
            type: 'boolean',
        },
        preserve_technical: {
            type: 'boolean',
        },
        allow_auto_paste: {
            type: 'boolean',
        },
        headless_playback: {
            type: 'boolean',
        },
        default_playback_voice_id: {
            type: 'any-of',
            contains: [{
                type: 'string',
            }, {
                type: 'null',
            }],
        },
        hotkey_enabled: {
            type: 'boolean',
        },
        chord_push_to_talk_keys: {
            type: 'array',
            contains: {
                type: 'string',
            },
        },
        chord_toggle_to_talk_keys: {
            type: 'array',
            contains: {
                type: 'string',
            },
        },
        chord_voice_stop_keys: {
            type: 'array',
            contains: {
                type: 'string',
            },
        },
        chord_voice_toggle_keys: {
            type: 'array',
            contains: {
                type: 'string',
            },
        },
        chord_agent_keys: {
            type: 'array',
            contains: {
                type: 'string',
            },
        },
    },
} as const;
