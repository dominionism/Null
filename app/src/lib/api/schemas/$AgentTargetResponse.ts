/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
export const $AgentTargetResponse = {
    description: `One live agent session, as herdr reports it.`,
    properties: {
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
        ready: {
            type: 'boolean',
            isRequired: true,
        },
        cwd: {
            type: 'any-of',
            contains: [{
                type: 'string',
            }, {
                type: 'null',
            }],
        },
        title: {
            type: 'any-of',
            contains: [{
                type: 'string',
            }, {
                type: 'null',
            }],
        },
        focused: {
            type: 'boolean',
        },
    },
} as const;
