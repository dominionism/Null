/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
import type { AgentListResponse } from '../models/AgentListResponse';
import type { VoiceDeliveryResponse } from '../models/VoiceDeliveryResponse';
import type { VoiceMessageRequest } from '../models/VoiceMessageRequest';
import type { CancelablePromise } from '../core/CancelablePromise';
import { OpenAPI } from '../core/OpenAPI';
import { request as __request } from '../core/request';
export class VoiceTargetsService {
    /**
     * List Agents
     * Agents herdr can see, for the settings picker and the pill.
     *
     * A missing herdr is a normal state, not an error: the response says so and
     * the caller falls back to pasting.
     * @returns AgentListResponse Successful Response
     * @throws ApiError
     */
    public static listAgentsAgentsGet(): CancelablePromise<AgentListResponse> {
        return __request(OpenAPI, {
            method: 'GET',
            url: '/agents',
        });
    }
    /**
     * Deliver Message
     * Send a transcript to the configured agent as a voice turn.
     *
     * The agent is re-listed before every send, so a target that has exited
     * reports itself instead of swallowing the words.
     * @returns VoiceDeliveryResponse Successful Response
     * @throws ApiError
     */
    public static deliverMessageVoiceTargetsMessagePost({
        requestBody,
    }: {
        requestBody: VoiceMessageRequest,
    }): CancelablePromise<VoiceDeliveryResponse> {
        return __request(OpenAPI, {
            method: 'POST',
            url: '/voice-targets/message',
            body: requestBody,
            mediaType: 'application/json',
            errors: {
                422: `Validation Error`,
            },
        });
    }
}
