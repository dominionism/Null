/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
export const $CloudStatusResponse = {
    description: `Current link between this device and a Voicebox Cloud account.`,
    properties: {
        connected: {
            type: 'boolean',
            isRequired: true,
        },
        device_name: {
            type: 'any-of',
            contains: [{
                type: 'string',
            }, {
                type: 'null',
            }],
        },
        account_user_id: {
            type: 'any-of',
            contains: [{
                type: 'string',
            }, {
                type: 'null',
            }],
        },
        key_prefix: {
            type: 'any-of',
            contains: [{
                type: 'string',
            }, {
                type: 'null',
            }],
        },
        connected_at: {
            type: 'any-of',
            contains: [{
                type: 'string',
                format: 'date-time',
            }, {
                type: 'null',
            }],
        },
        dashboard_url: {
            type: 'string',
            isRequired: true,
        },
    },
} as const;
