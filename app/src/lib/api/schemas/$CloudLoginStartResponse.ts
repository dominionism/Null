/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
export const $CloudLoginStartResponse = {
    description: `Returned when the desktop kicks off browser login. The backend has
    already opened the browser; the URL is included for fallback/debugging.`,
    properties: {
        authorize_url: {
            type: 'string',
            isRequired: true,
        },
    },
} as const;
