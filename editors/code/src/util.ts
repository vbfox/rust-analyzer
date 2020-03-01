import * as lc from "vscode-languageclient";
import * as vscode from "vscode";
import { strict as nativeAssert } from "assert";

export function assert(condition: boolean, explanation: string): asserts condition {
    try {
        nativeAssert(condition, explanation);
    } catch (err) {
        log.error(`Assertion failed:`, explanation);
        throw err;
    }
}

export const log = {
    enabled: true,
    debug(message?: any, ...optionalParams: any[]): void {
        if (!log.enabled) return;
        // eslint-disable-next-line no-console
        console.log(message, ...optionalParams);
    },
    error(message?: any, ...optionalParams: any[]): void {
        if (!log.enabled) return;
        debugger;
        // eslint-disable-next-line no-console
        console.error(message, ...optionalParams);
    },
    setEnabled(yes: boolean): void {
        log.enabled = yes;
    }
};

export async function sendRequestWithRetry<TParam, TRet>(
    client: lc.LanguageClient,
    reqType: lc.RequestType<TParam, TRet, unknown>,
    param: TParam,
    token?: vscode.CancellationToken,
): Promise<TRet> {
    for (const delay of [2, 4, 6, 8, 10, null]) {
        try {
            return await (token
                ? client.sendRequest(reqType, param, token)
                : client.sendRequest(reqType, param)
            );
        } catch (error) {
            if (delay === null) {
                log.error("LSP request timed out", { method: reqType.method, param, error });
                throw error;
            }

            if (error.code === lc.ErrorCodes.RequestCancelled) {
                throw error;
            }

            if (error.code !== lc.ErrorCodes.ContentModified) {
                log.error("LSP request failed", { method: reqType.method, param, error });
                throw error;
            }

            await sleep(10 * (1 << delay));
        }
    }
    throw 'unreachable';
}

function sleep(ms: number) {
    return new Promise(resolve => setTimeout(resolve, ms));
}

export function arrayShallowEqual<T>(a: ArrayLike<T>, b: ArrayLike<T>) {
    if (a === b) return true;
    if (a.length != b.length) return false;

    for (var i = 0; i < a.length; ++i) {
        if (a[i] !== b[i]) {
            return false;
        }
    }

    return true;
}