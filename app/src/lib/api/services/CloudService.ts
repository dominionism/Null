/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
import type { CloudLoginStartResponse } from '../models/CloudLoginStartResponse';
import type { CloudStatusResponse } from '../models/CloudStatusResponse';
import type { CancelablePromise } from '../core/CancelablePromise';
import { OpenAPI } from '../core/OpenAPI';
import { request as __request } from '../core/request';
export class CloudService {
    /**
     * Start Cloud Login
     * @returns CloudLoginStartResponse Successful Response
     * @throws ApiError
     */
    public static startCloudLoginCloudLoginStartPost(): CancelablePromise<CloudLoginStartResponse> {
        return __request(OpenAPI, {
            method: 'POST',
            url: '/cloud/login/start',
        });
    }
    /**
     * Cloud Callback
     * @returns string Successful Response
     * @throws ApiError
     */
    public static cloudCallbackCloudCallbackGet({
        code = '',
        state = '',
    }: {
        code?: string,
        state?: string,
    }): CancelablePromise<string> {
        return __request(OpenAPI, {
            method: 'GET',
            url: '/cloud/callback',
            query: {
                'code': code,
                'state': state,
            },
            errors: {
                422: `Validation Error`,
            },
        });
    }
    /**
     * Cloud Status
     * @returns CloudStatusResponse Successful Response
     * @throws ApiError
     */
    public static cloudStatusCloudStatusGet(): CancelablePromise<CloudStatusResponse> {
        return __request(OpenAPI, {
            method: 'GET',
            url: '/cloud/status',
        });
    }
    /**
     * Cloud Disconnect
     * @returns CloudStatusResponse Successful Response
     * @throws ApiError
     */
    public static cloudDisconnectCloudDisconnectPost(): CancelablePromise<CloudStatusResponse> {
        return __request(OpenAPI, {
            method: 'POST',
            url: '/cloud/disconnect',
        });
    }
}
