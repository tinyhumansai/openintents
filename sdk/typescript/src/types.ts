/** Types for the OpenIntents v1 API. Source of truth: spec/openapi.yaml. */

export type IntentStatus =
  | "queued"
  | "running"
  | "requires_payment"
  | "requires_input"
  | "requires_login"
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
  /** Always wait in `requires_payment` with a payment link (`reason: requested`) before paying. */
  require_payment_link?: boolean;
  /**
   * When the shop wants the payer signed in and asks for a phone number or
   * email and then a code: let the agent sign them in itself, asking for each
   * value in `requires_input` (`true`, the default), or never (`false`). Takes
   * effect only where OpenIntents has agent sign-in on for your key.
   */
  sign_in_agent?: boolean;
  /**
   * The payer pays the shop themselves (default `false`): cash on delivery
   * where the shop offers it (the order goes through, `paid_with`
   * `cash_on_delivery`), else the shop's UPI QR code: the intent waits in
   * `requires_input` with a `payment` request whose `image_url` is a picture
   * of the code and whose `upi_url`, when present, is the same payment as a
   * link to open in a UPI app; answer `done` once they paid, or decline. No
   * wallet is used, and the fee is waived for now.
   * Live keys only; where it is not available for your key, creating the
   * intent is a 503 `pay_at_shop_unavailable`.
   */
  pay_at_shop?: boolean;
  /**
   * The payer's phone number or email for signing in, so the agent need not
   * ask for it. Kept sealed, used once, deleted when the intent ends, and
   * never returned.
   */
  sign_in_hint?: SignInHint;
  /**
   * When the shop wants the payer signed in and the agent does not sign them
   * in itself: wait in `requires_login` with a sign-in link for them (`true`,
   * the default), or fail with `login_required` (`false`).
   */
  sign_in_link?: boolean;
  /** Up to 20 of your own key/value pairs. */
  metadata?: Record<string, string>;
  /** A customer of your account to buy for: their wallet and browser profile. */
  customer?: string;
  /**
   * Where the payer's browser goes out from: ISO 3166-1 alpha-2, lowercase
   * (`gb`, not `uk`). Without it: the country the payer last browsed from,
   * else the country of `end_user_ip`, else the default. A country no browser
   * can go out from is a 422 `country_unavailable` (`param: country`).
   */
  country?: string;
  /** The payer's own IP address, so their country can be inferred. Not stored. */
  end_user_ip?: string;
  /**
   * A URL that receives this intent's `intent.*` events as well as your
   * endpoints'. The create response alone carries its `webhook_secret`.
   */
  webhook_url?: string;
  /**
   * How long, in seconds (60 to 86400, default 86400), a question the agent
   * asks the payer may wait for its answer once the agent stops waiting with a
   * browser open.
   */
  input_ttl_seconds?: number;
  /**
   * Handles (`sec_...`) of the payer's secrets the purchase may use. Without
   * it, the payer's defaults; `[]` offers none.
   */
  secrets?: string[];
  /** Shops (domains) you let the purchase fill secrets on; `["*"]` lets it fill address and contact secrets on any shop. */
  shops?: string[];
}

/** Why a browser goes out from its `country`. Others may appear. */
export type CountrySource = "explicit" | "payer" | "end_user_ip" | "request_ip" | "default";

export interface PaymentRequest {
  reason:
    | "over_limit"
    | "requested"
    | "connect_wallet"
    | "add_money"
    | "identity_check"
    | "wallet_processing";
  payment_url: string;
  merchant?: string | null;
  total: number;
  fee: number;
  currency: string;
  expires_at: string;
}

/**
 * Where the payer signs in to a shop, so a waiting intent carries on. Send
 * `url` to the payer only: it works for 30 minutes, on the first device that
 * opens it.
 */
export interface SignInRequest {
  url: string;
  /** The shop's domain, such as `amazon.com`. */
  shop: string;
  expires_at: string;
}

/** The payer's phone number or email for the agent to sign in with. */
export interface SignInHint {
  /** Digits, with an optional leading `+`; spaces, dashes, dots and parentheses are fine. */
  phone?: string;
  email?: string;
}

/**
 * What an `InputRequest` needs: the payer's `phone` number or `email`, a
 * one-time `code` the shop sent them, their `approval` of the sign-in in the
 * shop's app or on another device (it has no value: answer `done`), or a
 * `detail`: anything else the purchase needs that the request did not give,
 * such as the email the shop wants for the order, a size or a colour. A
 * detail's `prompt` is the agent's own question (never asking for passwords,
 * codes, PINs, card or bank details or ID numbers), and the answer goes back
 * to the agent. A `sensitive` question wants an identity document, answered
 * on its `url` only. A `payment` asks the payer to pay the shop themselves
 * (`pay_at_shop`): show them `image_url`, a picture of the shop's payment code
 * to scan with a UPI app, and answer `done` once they paid (it has no value),
 * or decline. A `check` is a captcha or puzzle the shop shows: send the payer
 * to its `url`, where they finish it in a live view of the purchase's browser,
 * and answer `done` once they did (it has no value), or decline.
 */
export type InputKind =
  | "phone"
  | "email"
  | "code"
  | "approval"
  | "detail"
  | "sensitive"
  | "payment"
  | "check";

/** One of the choices a question offers, such as a seat, a fare or a delivery slot. */
export interface InputChoice {
  /** What an answer names; unique in the question. */
  id: string;
  label: string;
  description?: string;
  /** What choosing it adds to the price (negative, takes off), in minor units of `currency`. */
  price_delta?: number;
  currency?: string;
}

/** What a field of a form takes. */
export type InputFieldType = "text" | "email" | "phone" | "date" | "number" | "choice";

/** One field of a form a question asks the payer to fill in. */
export interface InputField {
  /** The key of this field's answer in `values`. */
  name: string;
  label: string;
  type: InputFieldType;
  required: boolean;
  /** For a `choice` field, its options; answer with an option's `id`. */
  options?: InputChoice[];
}

/**
 * What the agent needs from the payer while it signs them in to a shop
 * itself, or a `detail` the purchase needs. Ask them `prompt` and answer with
 * `intents.input` right away: it expires after 5 minutes. Or send them `url`
 * to type the answer there. An intent asks at most 3 details.
 */
export interface InputRequest {
  /** Answer with this ID. */
  id: string;
  kind: InputKind;
  /** The shop's registrable domain, such as `blinkit.com`. */
  shop: string;
  /**
   * What to ask the payer, written by OpenIntents (for a `detail`, the
   * agent's own question). Show it to them as it is.
   */
  prompt: string;
  /**
   * A `detail`'s choices (at most 8), empty otherwise. When not empty, answer
   * with one of them (case-insensitive): anything else is a 400.
   */
  options: string[];
  /**
   * A `detail` that offers choices with ids (2 to 50), empty otherwise.
   * Answer with the `id` of one in `value`; any other is a 422 `unknown_option`.
   */
  choices: InputChoice[];
  /** A `detail` that is a form (at most 10), empty otherwise. Answer every field at once with `values`. */
  fields: InputField[];
  /** What a plain answer must look like. */
  expects: "text" | "email" | "phone" | "choice";
  /** Where the shop sent the code, when known. */
  channel: "sms" | "email" | "call" | "app" | null;
  /** How many characters the code has, when the page says. */
  length: number | null;
  /** A page where the payer types the answer themselves. Theirs only: whoever has it can answer. */
  url: string;
  /**
   * A `payment`'s picture (JPEG) of the shop's payment code, for the payer to
   * scan; served only while the request waits, absent for other kinds. Like
   * `url`, whoever has it can see it.
   */
  image_url?: string;
  /**
   * A `payment`'s UPI payment link (`upi://pay?…`) for the same payment as the
   * code, when the shop's page gave one: open it in a UPI app on the phone
   * instead of scanning. Absent for other kinds, and when the page gave none.
   */
  upi_url?: string;
  expires_at: string;
  /** How many more times the agent may ask for this `kind` in this purchase, after this one. */
  attempts_left: number;
}

/**
 * An answer to an intent's `input_request`: its `id` as `input`, and one of
 * `value` (the phone number, email or code, or a `detail`'s answer: a line of
 * at most 300 characters, one of its `options`, or the `id` of one of its
 * `choices`), `values` (a form's answers by field `name`), `decline` (the
 * payer would rather not) or `done` (only for an `approval`, once they
 * approved it, or a `payment`, once they paid). A `sensitive` question takes only a decline: the payer
 * answers it on its `url` (anything else is a 422 `answer_on_page`).
 */
export type AnswerInputParams =
  | { input: string; value: string }
  | { input: string; values: Record<string, string> }
  | { input: string; decline: true }
  | { input: string; done: true };

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
  /**
   * How the shop was paid: `wallet` (the payer's own wallet), `cash_on_delivery`
   * or `upi_qr` (the payer themselves, `pay_at_shop`). Absent on older
   * receipts and test purchases.
   */
  paid_with?: "wallet" | "upi_qr" | "cash_on_delivery";
  /**
   * What was bought, where it is not an order of goods: `ride`, a ride
   * requested from a ride app, its product as the item and its fare as the
   * total. Absent on orders.
   */
  kind?: "ride";
  /**
   * How far it got, where `kind` says: a `ride` is `requested`, the app
   * looking for a driver, which none may have taken yet. Never a PIN or OTP.
   */
  state?: "requested";
}

export interface IntentError {
  /**
   * Why the intent did not finish: on a `failed` one, such as
   * `over_max_amount` or `needs_review`; on a `cancelled` one,
   * `may_be_placed` (cancelled after its order was placed at the shop,
   * which may have it).
   */
  code: string;
  message: string;
}

export interface Intent {
  id: string;
  object: "intent";
  status: IntentStatus;
  intent: string;
  /** The customer it buys for; `null` for your account's own intents. */
  customer?: string | null;
  max_amount?: number | null;
  currency?: string | null;
  context?: Record<string, unknown> | null;
  metadata?: Record<string, string> | null;
  /** Set when `status` is `requires_payment`. */
  payment?: PaymentRequest | null;
  /** Set when `status` is `requires_login`. */
  sign_in?: SignInRequest | null;
  /** Set when `status` is `requires_input`. */
  input_request?: InputRequest | null;
  /** Where this intent's own events go, if it was created with a `webhook_url`. */
  webhook_url?: string | null;
  /** Signs the `webhook_url` deliveries (`whsec_...`). Only in the create response. */
  webhook_secret?: string;
  /** The secrets the purchase may use, as given (never a value); `null` when none were named. */
  secrets?: string[] | null;
  /** The shops you let the purchase fill secrets on. */
  shops?: string[];
  /** The country its browser goes out from. */
  country?: string | null;
  country_source?: CountrySource | null;
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
  /** Only intents bought for this customer. */
  customer?: string;
  /** 1 to 100, default 20. */
  limit?: number;
  cursor?: string;
}

export interface IntentList {
  object: "list";
  data: Intent[];
  next_cursor?: string | null;
}

/** Someone you buy for, with their own wallet and browser profile. */
export interface Customer {
  id: string;
  object: "customer";
  /** Your own ID for them, unique in your account. */
  external_id?: string | null;
  name?: string | null;
  email?: string | null;
  metadata?: Record<string, string>;
  /** What they may spend. */
  limits?: Limits;
  created_at: string;
}

/**
 * What a customer may spend, in cents, counting the cart's total of each
 * purchase their wallet paid. `null` is no limit. Checked before anything is
 * paid; an intent over one fails with `limit_exceeded`.
 */
export interface Limits {
  per_intent: number | null;
  monthly: number | null;
}

/** Limits to set: one left out stays as it is; a number sets it; `null` removes it. Whole numbers from 1 to 1000000000. */
export interface LimitsParams {
  per_intent?: number | null;
  monthly?: number | null;
}

export interface CustomerParams {
  /** Unique in your account; one already in use is a 409 conflict. */
  external_id?: string;
  name?: string;
  email?: string;
  /** Up to 20 pairs. On update it replaces the whole map. */
  metadata?: Record<string, string>;
  /** What they may spend. */
  limits?: LimitsParams;
}

export interface ListCustomersParams {
  /** Only the customer with this `external_id`. */
  external_id?: string;
  /** 1 to 100, default 20. */
  limit?: number;
  cursor?: string;
}

export interface CustomerList {
  object: "list";
  data: Customer[];
  next_cursor?: string | null;
}

/**
 * One step in an intent's or a session's timeline. `type` says what `data`
 * holds: `status` (`{ status, reason? }`), `step` and `checkout` for intents,
 * `page` and `command` for sessions. New types may appear: skip the ones you
 * don't know.
 */
export interface Event {
  id: string;
  object: "event";
  type: string;
  created_at: string;
  data: Record<string, unknown>;
}

export interface EventList {
  object: "list";
  data: Event[];
  next_cursor?: string | null;
}

export interface ListEventsParams {
  /** 1 to 100, default 20. */
  limit?: number;
  cursor?: string;
}

export interface StreamEventsOptions {
  /** Start after this event (an event's `id`) instead of at the oldest. */
  lastEventId?: string;
  /** Stops the stream. */
  signal?: AbortSignal;
}

/**
 * `ready`: nothing uses it. `in_use`: an intent or a session holds it (see
 * `lease`). `forgetting`: being erased. `forgotten`: erased, and nothing saved
 * in it since.
 */
export type ProfileStatus = "ready" | "in_use" | "forgetting" | "forgotten";

/** What uses a profile now. A profile is used by one intent or session at a time. */
export interface ProfileLease {
  /** `type` is `intent` or `session`. */
  holder: { type: string; id: string };
  /** When the lease next runs out. It is renewed while the holder works. */
  expires_at: string;
}

/**
 * A payer's browser profile: the merchant logins, cookies and carts their
 * intents and sessions build up. Your account has one, and so does each
 * customer.
 */
export interface Profile {
  id: string;
  object: "profile";
  /** Its customer; `null` for your account's own profile. */
  customer?: string | null;
  status: ProfileStatus;
  lease?: ProfileLease | null;
  /** The saved profile's size; `null` before anything is saved. */
  size_bytes?: number | null;
  created_at: string;
  /** When an intent or a session last took it. */
  last_used_at?: string | null;
}

export interface CreateProfileParams {
  /** This customer's profile; without it, your account's. */
  customer?: string;
}

export interface ListProfilesParams {
  /** Only this customer's profile. */
  customer?: string;
  /** 1 to 100, default 20. */
  limit?: number;
  cursor?: string;
}

export interface ProfileList {
  object: "list";
  data: Profile[];
  next_cursor?: string | null;
}

export type SessionStatus = "queued" | "opening" | "open" | "closing" | "closed" | "failed";

export interface CreateSessionParams {
  /** The profile to open. Pass this or `customer`, not both; neither opens your account's own. */
  profile?: string;
  /** Open this customer's profile. */
  customer?: string;
  /** A public `https` URL to start on; without it, a blank page. */
  start_url?: string;
  /** 1 to 15 (the default), counted from when the session opens. */
  max_minutes?: number;
  /** See `CreateIntentParams.country`. */
  country?: string;
  /** See `CreateIntentParams.end_user_ip`. */
  end_user_ip?: string;
  /** A URL that receives this session's `session.*` events as well as your endpoints'. */
  webhook_url?: string;
}

/** A browser opened on a profile without an intent. */
export interface Session {
  id: string;
  object: "session";
  profile: string;
  customer?: string | null;
  status: SessionStatus;
  /** Opened with a test key. */
  test: boolean;
  start_url?: string | null;
  /** The page it is on now. */
  url?: string | null;
  title?: string | null;
  created_at: string;
  opened_at?: string | null;
  /** When it closes by itself, at the latest. */
  expires_at: string;
  closed_at?: string | null;
  /** The country its browser goes out from. */
  country?: string | null;
  country_source?: CountrySource | null;
  /** Where this session's own events go, if it was opened with a `webhook_url`. */
  webhook_url?: string | null;
  /** Signs the `webhook_url` deliveries (`whsec_...`). Only in the create response. */
  webhook_secret?: string;
  /** Why it ended: `released`, `idle`, `timed_out`, `data_limit`, `browser_lost` or `error`; others may appear. */
  close_reason?: string | null;
}

export interface ListSessionsParams {
  status?: SessionStatus;
  profile?: string;
  /** Only sessions on this customer's profile. */
  customer?: string;
  /** 1 to 100, default 20. */
  limit?: number;
  cursor?: string;
}

export interface SessionList {
  object: "list";
  data: Session[];
  next_cursor?: string | null;
}

/** Go to a public `https` URL; other hosts and ports are refused (`url_not_allowed`). */
export interface NavigateCommand {
  type: "navigate";
  url: string;
}

/** The page's accessibility tree, with a `ref` on each element you can act on. */
export interface SnapshotCommand {
  type: "snapshot";
}

/**
 * Act on an element by its `ref` from the latest snapshot. `type` works only
 * on text fields, with line breaks but no other control characters; `press`
 * takes one key, alone or with Shift or Control (`Enter`, `Shift+Tab`,
 * `Control+a`). After a `navigate`, and after any act except a `type`
 * without line breaks, take a new snapshot before the next act.
 */
export type ActCommand =
  | { type: "act"; ref: string; action: "click" }
  | { type: "act"; ref: string; action: "type"; text: string }
  | { type: "act"; ref: string; action: "select"; value: string }
  | { type: "act"; ref: string; action: "press"; key: string };

/** The page's readable text, or one element's with a CSS `selector`. */
export interface ReadCommand {
  type: "read";
  selector?: string;
}

/** One thing to do on a session's page. A session never pays or places an order. */
export type SessionCommand = NavigateCommand | SnapshotCommand | ActCommand | ReadCommand;

export type SessionCommandStatus = "pending" | "done" | "refused" | "failed";

/** Why a session command was `refused` or `failed`. */
export interface Failure {
  /** Such as `payment_not_allowed`, `url_not_allowed`, `not_found`, `not_actionable`, `session_closed`. */
  code: string;
  message: string;
}

export interface SessionCommandResult {
  id: string;
  object: "session_command";
  /** The command's `type`: `navigate`, `snapshot`, `act` or `read`. */
  type: string;
  status: SessionCommandStatus;
  /** The page after the command. */
  url?: string | null;
  title?: string | null;
  /** For `snapshot`, the tree. It never includes what text fields hold. */
  snapshot?: string | null;
  /** For `read`, the text. */
  text?: string | null;
  /** Why it was `refused` or `failed`. */
  error?: Failure | null;
  created_at: string;
  completed_at?: string | null;
}

export type IntentWebhookEventType =
  | "intent.running"
  | "intent.requires_payment"
  | "intent.requires_input"
  | "intent.requires_login"
  | "intent.completed"
  | "intent.failed"
  | "intent.cancelled";

/** Sent only to endpoints subscribed to `session.*` types. */
export type SessionWebhookEventType = "session.opened" | "session.closed" | "session.failed";

/** Sent only to endpoints subscribed to `sign_in.*` types. */
export type SignInWebhookEventType = "sign_in.completed" | "sign_in.expired";

/** Sent only to endpoints subscribed to it. */
export type SecretRequestWebhookEventType = "secret_request.completed";

export type WebhookEventType =
  | IntentWebhookEventType
  | SessionWebhookEventType
  | SignInWebhookEventType
  | SecretRequestWebhookEventType;

export interface IntentWebhookEvent {
  id: string;
  type: IntentWebhookEventType;
  created_at: string;
  data: Intent;
}

export interface SessionWebhookEvent {
  id: string;
  type: SessionWebhookEventType;
  created_at: string;
  data: Session;
}

/** A sign-in link ended. `data` is the sign-in without its `url`. */
export interface SignInWebhookEvent {
  id: string;
  type: SignInWebhookEventType;
  created_at: string;
  data: SignIn;
}

/** A customer filled in a secret request. `data` holds the handles of the secrets made, never a value. */
export interface SecretRequestWebhookEvent {
  id: string;
  type: SecretRequestWebhookEventType;
  created_at: string;
  data: SecretRequest;
}

/** What `webhookEndpoints.test` sends, to that endpoint alone. Not a type you can subscribe to. */
export interface WebhookTestEvent {
  id: string;
  type: "webhook_endpoint.test";
  created_at: string;
  data: { endpoint: string; message: string };
}

/** Dispatch on `type` before reading `data`. */
export type WebhookEvent =
  | IntentWebhookEvent
  | SessionWebhookEvent
  | SignInWebhookEvent
  | SecretRequestWebhookEvent
  | WebhookTestEvent;

/** A link to a customer's own page, shown once. Send `url` to the customer only. */
export interface PortalSession {
  id: string;
  object: "portal_session";
  customer: string;
  /** The country the customer's browser goes out from. */
  country: string;
  country_source: CountrySource;
  /** Works once, until `expires_at`. */
  url: string;
  expires_at: string;
}

export interface CreatePortalSessionParams {
  /** A public `https` URL the page offers a way back to. */
  return_url?: string;
  country?: string;
  end_user_ip?: string;
  /** How long the customer's session lasts once they open the link: 60 to 86400, default 28800 (8 hours). */
  ttl_seconds?: number;
}

/** What a secret holds, and so which `fields` it takes. */
export type SecretKind = "address" | "contact" | "identity_document" | "shop_password" | "loyalty";

export interface CreateSecretParams {
  /** The customer it is for; without it, your account's own. */
  customer?: string;
  kind: SecretKind;
  /** A name for it, up to 60 characters. Shown in lists and to agents: put no value in it. */
  label: string;
  /** The kind's fields, as text, up to 4 KB in all. */
  fields: Record<string, string>;
  /** Shops (domains) it may be used on. */
  shops?: string[];
  /** Offer this one first among the payer's secrets of its kind. */
  default?: boolean;
  /** After this time it is no longer offered and is deleted. */
  expires_at?: string;
}

/** A stored secret as you may see it: never its values. */
export interface Secret {
  id: string;
  object: "secret";
  /** The customer it belongs to; `null` for your account's own. */
  customer: string | null;
  kind: SecretKind;
  label: string;
  /** What a person may be shown to recognise it, such as `Paris, FR` for an address. */
  mask: string;
  /** The few fields its kind keeps in the clear. */
  public: Record<string, string>;
  shops: string[];
  default: boolean;
  /** The mode of the key that wrote it. A key sees only secrets of its own mode. */
  mode: "test" | "live";
  /** `developer` through this API, `payer` on their own page. */
  entered_by: "developer" | "payer";
  /** Goes up each time it is replaced. */
  version: number;
  created_at: string;
  updated_at: string;
  expires_at: string | null;
  last_used_at: string | null;
}

export interface ListSecretsParams {
  /** A customer's secrets instead of your account's. */
  customer?: string;
  kind?: SecretKind;
}

export interface SecretList {
  object: "list";
  data: Secret[];
  next_cursor?: string | null;
}

/** One row of a secret's history. Never a value or a mask. */
export interface SecretUse {
  id: string;
  object: "secret_use";
  secret: string;
  kind: SecretKind;
  /** `create`, `replace`, `delete`, `expire`, `fill`, `fund` or `refuse`; others may appear. */
  action: string;
  /** `worker`, `api`, `payer`, `developer` or `system`. */
  actor: string;
  outcome: "ok" | "refused";
  /** Why a use was refused, such as `shop_not_allowed`, `expired` or `wrong_use`. */
  reason?: string | null;
  field?: string | null;
  /** The names of the fields a create or replace set. */
  field_names: string[];
  shop?: string | null;
  intent?: string | null;
  at: string;
}

export interface ListSecretUsesParams {
  /** 1 to 100, default 20. */
  limit?: number;
  cursor?: string;
}

export interface SecretUseList {
  object: "list";
  data: SecretUse[];
  next_cursor?: string | null;
}

export interface CreateSecretRequestParams {
  /** The kinds of secret the page asks for. */
  kinds: SecretKind[];
  /** A public `https` URL the page offers a way back to. */
  return_url?: string;
  /** How long the link works: 60 to 604800, default 86400. */
  ttl_seconds?: number;
}

/** A request for a customer's secrets, with the link to send them. */
export interface SecretRequestCreated {
  id: string;
  object: "secret_request";
  customer: string;
  kinds: SecretKind[];
  /** For the customer only. Works once, until `expires_at`. */
  url: string;
  expires_at: string;
}

/** Where a request for a customer's secrets stands; also the `data` of `secret_request.completed`. */
export interface SecretRequest {
  id: string;
  object: "secret_request";
  customer: string;
  mode: "test" | "live";
  kinds: SecretKind[];
  /** `pending` until the customer fills it in, then `done`; `expired` if they did not in time. */
  state: "pending" | "done" | "expired";
  expires_at: string;
  completed_at: string | null;
  /** The handles (`sec_...`) of the secrets the customer made. */
  secrets: string[];
}

/** A link to a session's live view, shown once. Whoever holds `url` can drive the session. */
export interface LiveLink {
  object: "live_link";
  session: string;
  url: string;
  /** 10 minutes after it was made. */
  expires_at: string;
}

export interface CreateSignInParams {
  /** A public `https` URL on the shop, on the default port. The page opens there. */
  shop_url: string;
  /** Sign this customer in; without it, your account's own profile. */
  customer?: string;
  country?: string;
  end_user_ip?: string;
  /** A public `https` URL the page offers a way back to. */
  return_url?: string;
}

/**
 * `open` until the payer opens it, `active` while their browser is open, then
 * `done` (they signed in), `declined`, `expired` or `cancelled`.
 */
export type SignInStatus = "open" | "active" | "done" | "declined" | "expired" | "cancelled";

/** A link where a payer signs in to a shop with no intent attached. */
export interface SignIn {
  id: string;
  object: "sign_in";
  status: SignInStatus;
  customer: string | null;
  /** The shop's domain, such as `amazon.com`. */
  shop: string;
  country: string | null;
  country_source: CountrySource | null;
  return_url: string | null;
  /** The page for the payer only. Not in webhooks. */
  url?: string;
  test: boolean;
  created_at: string;
  expires_at: string;
  ended_at: string | null;
}

export interface WebhookEndpoint {
  id: string;
  object: "webhook_endpoint";
  url: string;
  /** The event types it receives. Empty: every intent event. */
  events: string[];
  disabled: boolean;
  created_at: string;
  /** Present while the secret before the last rotation still signs as well. */
  previous_secret_expires_at?: string;
  /** The signing secret (`whsec_...`). Only in the create and rotate responses. */
  secret?: string;
}

export interface CreateWebhookEndpointParams {
  /** `https` on the default port, resolving to a public address. */
  url: string;
  /** Event types to receive, such as `intent.completed` or `sign_in.completed`. Without any, every intent event. */
  events?: string[];
}

export interface UpdateWebhookEndpointParams {
  url?: string;
  /** Replaces the list; `[]` is every intent event. */
  events?: string[];
  disabled?: boolean;
}

export interface ListWebhookEndpointsParams {
  /** 1 to 100, default 20. */
  limit?: number;
  cursor?: string;
}

export interface WebhookEndpointList {
  object: "list";
  data: WebhookEndpoint[];
  next_cursor?: string | null;
}

/** One send of an event to an endpoint. */
export interface WebhookDelivery {
  id: string;
  object: "webhook_delivery";
  endpoint: string | null;
  event_id: string;
  event_type: string;
  /** `pending` while it is being tried (and retried), then `delivered` or `failed`. */
  status: "pending" | "delivered" | "failed";
  attempts: number;
  /** What your receiver answered on the last attempt, if it answered. */
  response_code: number | null;
  /** Why the last attempt failed, if it got no answer. */
  error: string | null;
  created_at: string;
  /** The next attempt. Only while `pending`. */
  next_attempt_at?: string;
  delivered_at: string | null;
  /** The delivery this one sends again. */
  redelivery_of: string | null;
}

export interface ListWebhookDeliveriesParams {
  /** 1 to 100, default 20. */
  limit?: number;
  cursor?: string;
}

export interface WebhookDeliveryList {
  object: "list";
  data: WebhookDelivery[];
  next_cursor?: string | null;
}
