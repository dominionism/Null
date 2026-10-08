/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
export const $VoiceDeliveryResponse = {
    description: `Where a transcript landed.`,
    properties: {
        delivered: {
            type: 'boolean',
            isRequired: true,
        },
        target: {
            type: 'string',
            isRequired: true,
        },
        agent: {
            type: 'string',
            isRequired: true,
        },
        status: {
            type: 'string',
            isRequired: true,
        },
    },
} as const;
