/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
export const $VoiceMessageRequest = {
    description: `A spoken transcript to deliver to an agent session.`,
    properties: {
        text: {
            type: 'string',
            isRequired: true,
        },
        target: {
            type: 'any-of',
            contains: [{
                type: 'string',
            }, {
                type: 'null',
            }],
        },
        voice_turn: {
            type: 'boolean',
        },
    },
} as const;
