/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
export const $AgentListResponse = {
    description: `Live agents. \`\`available\`\` is false when herdr is absent or unhappy.`,
    properties: {
        available: {
            type: 'boolean',
            isRequired: true,
        },
        agents: {
            type: 'array',
            contains: {
                type: 'AgentTargetResponse',
            },
        },
        error: {
            type: 'any-of',
            contains: [{
                type: 'string',
            }, {
                type: 'null',
            }],
        },
    },
} as const;
