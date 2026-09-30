/** Types for the OpenIntents v1 API. Source of truth: spec/openapi.yaml. */

export type IntentStatus =
  | "queued"
  | "running"
  | "requires_payment"
  | "completed"
  | "failed"
  | "cancelled";

/** Statuses an intent never leaves. */
export const FINAL_STATUSES: readonly IntentStatus[] = ["completed", "failed", "cancelled"];

export interface CreateIntentParams {
  /** What to buy or book, in natural language. */
  intent: string;
  /** Spending cap in cents; the intent fails rather than exceed it. */
  max_amount?: number;
  /** ISO currency code, lowercase. */
  currency?: string;
  /** Structured details: addresses, names, loyalty numbers. */
  context?: Record<string, unknown>;
  /** Always return a payment link instead of paying from the balance. */
  require_payment_link?: boolean;
  /** Up to 20 of your own key/value pairs. */
  metadata?: Record<string, string>;
}

export interface PaymentRequest {
  reason: "insufficient_balance" | "over_limit" | "requested";
  payment_url: string;
  merchant?: string;
  total: number;
  fee: number;
  currency: string;
  expires_at: string;
}

export interface ReceiptItem {
  name: string;
  quantity: number;
  amount: number;
}

export interface Receipt {
  merchant: string;
  items?: ReceiptItem[];
  total: number;
  fee: number;
  currency: string;
  confirmation?: string | null;
  receipt_url?: string | null;
}

export interface IntentError {
  code: string;
  message: string;
}

export interface Intent {
  id: string;
  object: "intent";
  status: IntentStatus;
  intent: string;
  max_amount?: number | null;
  currency?: string | null;
  context?: Record<string, unknown> | null;
  metadata?: Record<string, string> | null;
  /** Set when `status` is `requires_payment`. */
  payment?: PaymentRequest | null;
  /** Set when `status` is `completed`. */
  result?: Receipt | null;
  /** Set when `status` is `failed`. */
  error?: IntentError | null;
  test: boolean;
  created_at: string;
  completed_at?: string | null;
}

export interface ListIntentsParams {
  status?: IntentStatus;
  /** 1 to 100, default 20. */
  limit?: number;
  cursor?: string;
}

export interface IntentList {
  object: "list";
  data: Intent[];
  next_cursor?: string | null;
}

export interface Balance {
  object: "balance";
  available: number;
  currency: string;
  monthly_limit?: number | null;
  spent_this_month?: number;
}

export type WebhookEventType =
  | "intent.running"
  | "intent.requires_payment"
  | "intent.completed"
  | "intent.failed"
  | "intent.cancelled";

export interface WebhookEvent {
  id: string;
  type: WebhookEventType;
  created_at: string;
  data: Intent;
}
