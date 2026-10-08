/* generated using openapi-typescript-codegen -- do not edit */
/* istanbul ignore file */
/* tslint:disable */
/* eslint-disable */
/**
 * Current link between this device and a Voicebox Cloud account.
 */
export type CloudStatusResponse = {
    connected: boolean;
    device_name?: (string | null);
    account_user_id?: (string | null);
    key_prefix?: (string | null);
    connected_at?: (string | null);
    dashboard_url: string;
};

