import type { ProfileLease } from "./types.js";

/** Error types returned by the API (`error.type` in the response body). */
export type ApiErrorType =
  | "invalid_request"
  | "authentication"
  | "payment"
  | "not_found"
  | "conflict"
  | "rate_limit"
  | "api_error";

/** The `RateLimit-*` headers of a response: what your key may do a minute and how much of it is left. */
export interface RateLimit {
  /** Requests a minute your key may make; also the burst. */
  limit: number;
  /** Requests left in your bucket. */
  remaining: number;
  /** Seconds until your bucket is full again. */
  reset: number;
}

/** Any non-2xx response from the API. */
export class OpenIntentsError extends Error {
  readonly status: number;
  readonly type: ApiErrorType | "network";
  /** A finer reason, such as `profile_busy`, `session_limit` or `session_not_open`. */
  readonly code?: string;
  readonly param?: string;
  /** With `profile_busy`: what uses the profile and when its lease next runs out. */
  readonly lease?: ProfileLease;
  readonly requestId?: string;
  /** With a 429 (or any answer that sends `Retry-After`): whole seconds to wait before the request can pass. */
  readonly retryAfter?: number;
  /** Your key's rate limit, when the response carried the `RateLimit-*` headers. */
  readonly rateLimit?: RateLimit;

  constructor(opts: {
    message: string;
    status: number;
    type: ApiErrorType | "network";
    code?: string;
    param?: string;
    lease?: ProfileLease;
    requestId?: string;
    retryAfter?: number;
    rateLimit?: RateLimit;
  }) {
    super(opts.message);
    this.name = "OpenIntentsError";
    this.status = opts.status;
    this.type = opts.type;
    this.code = opts.code;
    this.param = opts.param;
    this.lease = opts.lease;
    this.requestId = opts.requestId;
    this.retryAfter = opts.retryAfter;
    this.rateLimit = opts.rateLimit;
  }
}

/** `wait()` gave up before the intent reached a final status or needed payment. */
export class WaitTimeoutError extends Error {
  constructor(readonly intentId: string, readonly timeoutMs: number) {
    super(`Intent ${intentId} did not settle within ${timeoutMs}ms`);
    this.name = "WaitTimeoutError";
  }
}
