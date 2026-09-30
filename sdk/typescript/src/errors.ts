/** Error types returned by the API (`error.type` in the response body). */
export type ApiErrorType =
  | "invalid_request"
  | "authentication"
  | "payment"
  | "not_found"
  | "conflict"
  | "rate_limit"
  | "api_error";

/** Any non-2xx response from the API. */
export class OpenIntentsError extends Error {
  readonly status: number;
  readonly type: ApiErrorType | "network";
  readonly code?: string;
  readonly param?: string;
  readonly requestId?: string;

  constructor(opts: {
    message: string;
    status: number;
    type: ApiErrorType | "network";
    code?: string;
    param?: string;
    requestId?: string;
  }) {
    super(opts.message);
    this.name = "OpenIntentsError";
    this.status = opts.status;
    this.type = opts.type;
    this.code = opts.code;
    this.param = opts.param;
    this.requestId = opts.requestId;
  }
}

/** `wait()` gave up before the intent reached a final status or needed payment. */
export class WaitTimeoutError extends Error {
  constructor(readonly intentId: string, readonly timeoutMs: number) {
    super(`Intent ${intentId} did not settle within ${timeoutMs}ms`);
    this.name = "WaitTimeoutError";
  }
}
