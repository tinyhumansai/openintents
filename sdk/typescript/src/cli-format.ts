/** How the `openintents` command prints what the API returns. */
import type {
  Customer,
  Intent,
  LiveLink,
  PortalSession,
  Profile,
  Secret,
  SecretRequestCreated,
  Session,
  SessionCommandResult,
  SignIn,
} from "./types.js";

export const money = (cents: number, currency = "usd") =>
  new Intl.NumberFormat("en-US", { style: "currency", currency: currency.toUpperCase() }).format(cents / 100);

export function describeIntent(intent: Intent): string {
  const lines = [`${intent.id}  ${intent.status}  "${intent.intent}"`];
  if (intent.payment) {
    lines.push(`  payment needed: ${money(intent.payment.total, intent.payment.currency)} (+${money(intent.payment.fee, intent.payment.currency)} fee)`);
    lines.push(`  pay here: ${intent.payment.payment_url}`);
  }
  if (intent.sign_in) {
    lines.push(`  sign-in needed at ${intent.sign_in.shop}: send this link to the payer only`);
    lines.push(`  sign in here: ${intent.sign_in.url}`);
  }
  if (intent.input_request) {
    const r = intent.input_request;
    const done = r.kind === "approval" || r.kind === "payment" || r.kind === "check";
    const answer = done ? "--done" : "--value <answer>";
    lines.push(`  input needed at ${r.shop}, ask the payer: ${r.prompt}`);
    if (r.image_url) lines.push(`  payment code to scan (theirs only): ${r.image_url}`);
    if (r.options?.length) lines.push(`  options: ${r.options.join(", ")}`);
    lines.push(`  answer: openintents input ${intent.id} --input ${r.id} ${answer} (or --decline)`);
    lines.push(`  or the payer answers here (theirs only): ${r.url}`);
  }
  if (intent.result) {
    const r = intent.result;
    lines.push(`  ${r.merchant}: ${money(r.total, r.currency)} (+${money(r.fee, r.currency)} fee)${r.confirmation ? `, confirmation ${r.confirmation}` : ""}`);
    if (r.kind && r.state) lines.push(`  ${r.kind} ${r.state}`);
  }
  if (intent.error) lines.push(`  error ${intent.error.code}: ${intent.error.message}`);
  return lines.join("\n");
}

export function describeCustomer(c: Customer): string {
  const who = [c.name, c.email].filter(Boolean).join(" ");
  const lines = [`${c.id}  ${c.external_id ?? "-"}${who ? `  ${who}` : ""}`];
  const l = c.limits;
  if (l && (l.per_intent !== null || l.monthly !== null)) {
    const cap = (v: number | null) => (v === null ? "none" : money(v));
    lines.push(`  limits: ${cap(l.per_intent)} per intent, ${cap(l.monthly)} a month`);
  }
  return lines.join("\n");
}

export function describePortal(p: PortalSession): string {
  return [`${p.id}  portal for ${p.customer}, goes out from ${p.country}`, `  send this link to the customer only (works once, until ${p.expires_at}): ${p.url}`].join("\n");
}

export function describeSession(s: Session): string {
  const lines = [`${s.id}  ${s.status}  profile ${s.profile}${s.url ? `  at ${s.url}` : ""}`];
  if (s.close_reason) lines.push(`  closed: ${s.close_reason}`);
  return lines.join("\n");
}

export function describeLiveLink(l: LiveLink): string {
  return [`${l.session}  live view`, `  whoever holds this link can drive the session (until ${l.expires_at}): ${l.url}`].join("\n");
}

export function describeCommand(r: SessionCommandResult): string {
  const lines = [`${r.id}  ${r.type}  ${r.status}${r.url ? `  at ${r.url}` : ""}`];
  if (r.error) lines.push(`  ${r.error.code}: ${r.error.message}`);
  if (r.snapshot) lines.push(r.snapshot);
  if (r.text) lines.push(r.text);
  return lines.join("\n");
}

export function describeProfile(p: Profile): string {
  return `${p.id}  ${p.status}  ${p.customer ? `customer ${p.customer}` : "your account"}`;
}

export function describeSignIn(s: SignIn): string {
  const lines = [`${s.id}  ${s.status}  ${s.shop}`];
  if (s.url) lines.push(`  send this link to the payer only: ${s.url}`);
  return lines.join("\n");
}

/** A secret as a person may see it: its handle, kind, name and mask. Never a value. */
export function describeSecret(s: Secret): string {
  const lines = [`${s.id}  ${s.kind}  ${s.label}  ${s.mask}${s.default ? "  (default)" : ""}`];
  if (s.shops.length) lines.push(`  shops: ${s.shops.join(", ")}`);
  return lines.join("\n");
}

export function describeSecretRequest(r: SecretRequestCreated): string {
  return [`${r.id}  asks ${r.customer} for ${r.kinds.join(", ")}`, `  send this link to the customer only (until ${r.expires_at}): ${r.url}`].join("\n");
}
