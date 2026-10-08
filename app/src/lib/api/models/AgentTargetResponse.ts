/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
/**
 * One live agent session, as herdr reports it.
 */
export type AgentTargetResponse = {
    target: string;
    agent: string;
    status: string;
    ready: boolean;
    cwd?: (string | null);
    title?: (string | null);
    focused?: boolean;
};

