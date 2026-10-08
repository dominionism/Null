/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
import type { AgentTargetResponse } from './AgentTargetResponse';
/**
 * Live agents. ``available`` is false when herdr is absent or unhappy.
 */
export type AgentListResponse = {
    available: boolean;
    agents?: Array<AgentTargetResponse>;
    error?: (string | null);
};

