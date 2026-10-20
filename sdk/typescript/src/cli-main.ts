/**
 * The `openintents` command: a thin client for the REST API, for
 * shell-driven agents and for people trying intents out. `cli.ts` wires it to
 * the process; tests drive `runCli` with a scripted API.
 */
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname } from "node:path";
import { parseArgs } from "node:util";

import type { OpenIntents } from "./client.js";
import {
  describeCommand,
  describeCustomer,
  describeIntent,
  describeLiveLink,
  describePortal,
  describeProfile,
  describeSecret,
  describeSecretRequest,
  describeSession,
  describeSignIn,
} from "./cli-format.js";
import { OpenIntentsError, WaitTimeoutError } from "./errors.js";
import { mcpUrl, runMcpBridge } from "./mcp.js";
import type {
  AnswerInputParams,
  CustomerParams,
  IntentStatus,
  LimitsParams,
  SecretKind,
  SessionCommand,
  SessionStatus,
} from "./types.js";
import { VERSION } from "./version.js";

export const DASHBOARD_KEYS_URL = "https://openintents.io/dashboard/api-keys";

/** Exit codes, as documented: scripts can branch on the outcome. */
const EXIT: Record<IntentStatus | "error", number> = {
  completed: 0,
  queued: 0,
  running: 0,
  failed: 1,
  error: 1,
  requires_payment: 2,
  cancelled: 3,
  requires_login: 4,
  requires_input: 5,
};

/** What the command needs from its surroundings; tests swap these. */
export interface CliIo {
  client: () => Promise<OpenIntents>;
  /** The API key, for the `mcp` bridge. */
  apiKey?: () => Promise<string>;
  out: (line: string) => void;
  err: (line: string) => void;
  /** What the user types or pipes in: one line on a terminal, everything otherwise. */
  stdin: () => Promise<string>;
  openUrl: (url: string) => void;
  /** Where `login` saves the key. */
  configFile: string;
}

const INPUT_USAGE = "Usage: openintents input <id> --input <input id> (--value <answer> | --decline | --done)";

export const helpText = (configFile: string) => `openintents ${VERSION}: the agentic API to buy anything

Usage:
  openintents run "<intent>" [--max <dollars>] [--context key=value]... [--payment-link] [--wait]
  openintents status <id> [--wait]
  openintents list [--status <status>] [--limit <n>]
  openintents cancel <id>
  openintents input <id> --input <input id> (--value <answer> | --decline | --done)
                                answer what the intent asks the payer for
  openintents pay <id>          open the intent's payment link

  openintents customers create|update [<id>] [--external-id <id>] [--name <name>] [--email <email>]
                                [--metadata key=value]... [--per-intent <dollars|none>] [--monthly <dollars|none>]
  openintents customers get <id>
  openintents customers list [--external-id <id>] [--limit <n>] [--cursor <c>]
  openintents customers portal <id> [--return-url <url>] [--country <cc>] [--end-user-ip <ip>] [--ttl <seconds>]
                                a link to the customer's own page (send it to them only)

  openintents sessions open [--profile <id> | --customer <id>] [--start-url <url>] [--max-minutes <n>]
                                [--country <cc>] [--end-user-ip <ip>] [--webhook-url <url>]
  openintents sessions list [--status <status>] [--profile <id>] [--customer <id>] [--limit <n>] [--cursor <c>]
  openintents sessions get|release|live <id>
  openintents sessions command <id> navigate <url> | snapshot | read [<selector>] | click <ref>
                                | type <ref> <text> | select <ref> <value> | press <ref> <key>
  openintents sessions screenshot <id> --out <file.jpg>

  openintents profiles list [--customer <id>] [--limit <n>] [--cursor <c>]
  openintents profiles create [--customer <id>]
  openintents profiles get|forget <id>

  openintents sign-ins create <shop url> [--customer <id>] [--country <cc>] [--end-user-ip <ip>] [--return-url <url>]
  openintents sign-ins get <id>

  openintents secrets set --kind <kind> --label <name> (--field name=value... | --stdin)
                                [--customer <id>] [--shop <domain>]... [--default] [--expires-at <time>]
                                --stdin reads the fields as a JSON object, to keep them out of the process list
  openintents secrets list [--customer <id>] [--kind <kind>]
  openintents secrets get|delete <id>
  openintents secrets request <customer id> --kind <kind>... [--return-url <url>] [--ttl <seconds>]
                                a link where the customer enters their own (send it to them only)
  Stored secrets are write-only: no command shows a value.

  openintents mcp [--url <endpoint>]
                                stdio MCP server for clients without HTTP MCP: relays
                                to ${mcpUrl({})} with your key
  openintents login             save an API key to ${configFile}

Every command takes --json for one JSON object per line.
Auth: OPENINTENTS_API_KEY, or the key saved by \`openintents login\`.
Exit codes: 0 completed, 1 failed or error, 2 needs payment, 3 cancelled,
4 needs the payer to sign in, 5 needs an answer from the payer.`;

/** Dollars to cents, as the API counts: `"12.5"` is 1250. */
function cents(flag: string, text: string): number {
  const dollars = Number(text);
  if (!text.trim() || !Number.isFinite(dollars) || dollars <= 0) {
    throw new Error(`${flag} expects an amount in dollars, got "${text}"`);
  }
  return Math.round(dollars * 100);
}

function limit(flag: string, text: string): number | null {
  return text === "none" ? null : cents(flag, text);
}

function int(flag: string, text: string): number {
  const n = Number(text);
  if (!Number.isInteger(n)) throw new Error(`${flag} expects a whole number, got "${text}"`);
  return n;
}

/** `key=value` pairs. Errors name the key only, never the value. */
function pairs(flag: string, items: string[] | undefined): Record<string, string> {
  return Object.fromEntries(
    (items ?? []).map((kv) => {
      const i = kv.indexOf("=");
      if (i < 1) throw new Error(`${flag} expects name=value`);
      return [kv.slice(0, i), kv.slice(i + 1)];
    }),
  );
}

const defined = <T extends object>(o: T): T =>
  Object.fromEntries(Object.entries(o).filter(([, v]) => v !== undefined)) as T;

const OPTIONS = {
  json: { type: "boolean", default: false },
  wait: { type: "boolean", default: false },
  max: { type: "string" },
  context: { type: "string", multiple: true },
  "payment-link": { type: "boolean", default: false },
  status: { type: "string" },
  limit: { type: "string" },
  cursor: { type: "string" },
  url: { type: "string" },
  input: { type: "string" },
  value: { type: "string" },
  decline: { type: "boolean", default: false },
  done: { type: "boolean", default: false },
  customer: { type: "string" },
  "external-id": { type: "string" },
  name: { type: "string" },
  email: { type: "string" },
  metadata: { type: "string", multiple: true },
  "per-intent": { type: "string" },
  monthly: { type: "string" },
  "return-url": { type: "string" },
  country: { type: "string" },
  "end-user-ip": { type: "string" },
  ttl: { type: "string" },
  profile: { type: "string" },
  "start-url": { type: "string" },
  "max-minutes": { type: "string" },
  "webhook-url": { type: "string" },
  out: { type: "string" },
  kind: { type: "string", multiple: true },
  label: { type: "string" },
  field: { type: "string", multiple: true },
  stdin: { type: "boolean", default: false },
  shop: { type: "string", multiple: true },
  default: { type: "boolean", default: false },
  "expires-at": { type: "string" },
  help: { type: "boolean", short: "h", default: false },
  version: { type: "boolean", short: "v", default: false },
} as const;

type Parsed = ReturnType<typeof parseArgs<{ options: typeof OPTIONS; allowPositionals: true }>>["values"];

function customerParams(values: Parsed): CustomerParams {
  const metadata = pairs("--metadata", values.metadata);
  const limits: LimitsParams = defined({
    per_intent: values["per-intent"] === undefined ? undefined : limit("--per-intent", values["per-intent"]),
    monthly: values.monthly === undefined ? undefined : limit("--monthly", values.monthly),
  });
  return defined({
    external_id: values["external-id"],
    name: values.name,
    email: values.email,
    metadata: Object.keys(metadata).length ? metadata : undefined,
    limits: Object.keys(limits).length ? limits : undefined,
  });
}

function sessionCommand(words: string[]): SessionCommand {
  const [kind, a, b] = words;
  const need = (usage: string, ...args: Array<string | undefined>): void => {
    if (args.some((x) => x === undefined)) throw new Error(`Usage: openintents sessions command <id> ${usage}`);
  };
  switch (kind) {
    case "navigate":
      need("navigate <url>", a);
      return { type: "navigate", url: a! };
    case "snapshot":
      return { type: "snapshot" };
    case "read":
      return a === undefined ? { type: "read" } : { type: "read", selector: a };
    case "click":
      need("click <ref>", a);
      return { type: "act", ref: a!, action: "click" };
    case "type":
      need("type <ref> <text>", a, b);
      return { type: "act", ref: a!, action: "type", text: b! };
    case "select":
      need("select <ref> <value>", a, b);
      return { type: "act", ref: a!, action: "select", value: b! };
    case "press":
      need("press <ref> <key>", a, b);
      return { type: "act", ref: a!, action: "press", key: b! };
    default:
      throw new Error("Usage: openintents sessions command <id> navigate <url> | snapshot | read [<selector>] | click <ref> | type <ref> <text> | select <ref> <value> | press <ref> <key>");
  }
}

/** Runs one command line and returns the process exit code. */
export async function runCli(argv: string[], io: CliIo): Promise<number> {
  const json = argv.includes("--json");
  try {
    return await dispatch(argv, io);
  } catch (err) {
    const message =
      err instanceof OpenIntentsError
        ? `${err.message} (${err.type}${err.code ? `/${err.code}` : ""})`
        : err instanceof WaitTimeoutError
          ? err.message
          : (err as Error).message;
    if (json) io.out(JSON.stringify({ error: { message } }));
    else io.err(`openintents: ${message}`);
    return EXIT.error;
  }
}

async function dispatch(argv: string[], io: CliIo): Promise<number> {
  const { values, positionals } = parseArgs({ args: argv, allowPositionals: true, options: OPTIONS });
  const [command, sub, ...rest] = positionals;
  const json = values.json ?? false;
  const print = (value: unknown, human: () => string) => io.out(json ? JSON.stringify(value) : human());
  const printList = <T>(items: T[], one: (x: T) => string, empty: string) => {
    if (json) items.forEach((x) => io.out(JSON.stringify(x)));
    else io.out(items.map(one).join("\n") || empty);
  };
  const page = () => ({ limit: values.limit ? int("--limit", values.limit) : undefined, cursor: values.cursor });
  const country = () => defined({ country: values.country, end_user_ip: values["end-user-ip"] });
  const id = (usage: string): string => {
    if (!sub || command === undefined) throw new Error(`Usage: openintents ${usage}`);
    return sub;
  };

  if (values.version) {
    io.out(VERSION);
    return 0;
  }
  if (values.help || !command) {
    io.out(helpText(io.configFile));
    return command || values.help ? 0 : 1;
  }

  switch (command) {
    case "run": {
      if (!sub) throw new Error('Usage: openintents run "<intent>"');
      const context = pairs("--context", values.context);
      const oi = await io.client();
      let intent = await oi.intents.create({
        intent: sub,
        max_amount: values.max ? cents("--max", values.max) : undefined,
        context: Object.keys(context).length ? context : undefined,
        require_payment_link: values["payment-link"] || undefined,
      });
      if (values.wait) intent = await oi.intents.wait(intent.id);
      print(intent, () => describeIntent(intent));
      return EXIT[intent.status];
    }
    case "status": {
      if (!sub) throw new Error("Usage: openintents status <id>");
      const oi = await io.client();
      const intent = values.wait ? await oi.intents.wait(sub) : await oi.intents.get(sub);
      print(intent, () => describeIntent(intent));
      return EXIT[intent.status];
    }
    case "list": {
      const intents = await (await io.client()).intents.list({
        status: values.status as IntentStatus | undefined,
        limit: values.limit ? int("--limit", values.limit) : undefined,
      });
      printList(intents.data, describeIntent, "No intents.");
      return 0;
    }
    case "cancel": {
      const intent = await (await io.client()).intents.cancel(id("cancel <id>"));
      print(intent, () => describeIntent(intent));
      return EXIT[intent.status];
    }
    case "input": {
      const input = values.input;
      const given = [values.value !== undefined, values.decline, values.done].filter(Boolean).length;
      if (!sub || !input || given !== 1) throw new Error(INPUT_USAGE);
      const params: AnswerInputParams =
        values.value !== undefined
          ? { input, value: values.value }
          : values.decline
            ? { input, decline: true }
            : { input, done: true };
      const intent = await (await io.client()).intents.input(sub, params);
      print(intent, () => describeIntent(intent));
      return EXIT[intent.status];
    }
    case "pay": {
      const intent = await (await io.client()).intents.get(id("pay <id>"));
      if (intent.status !== "requires_payment" || !intent.payment) {
        throw new Error(`${intent.id} doesn't need payment (status: ${intent.status}).`);
      }
      const payment = intent.payment;
      if (!json) io.openUrl(payment.payment_url);
      print(payment, () => `Opened ${payment.payment_url}`);
      return 2;
    }
    case "customers": {
      const oi = await io.client();
      switch (sub) {
        case "create": {
          const c = await oi.customers.create(customerParams(values));
          print(c, () => describeCustomer(c));
          return 0;
        }
        case "update": {
          if (!rest[0]) throw new Error("Usage: openintents customers update <id> [flags]");
          const c = await oi.customers.update(rest[0], customerParams(values));
          print(c, () => describeCustomer(c));
          return 0;
        }
        case "get": {
          if (!rest[0]) throw new Error("Usage: openintents customers get <id>");
          const c = await oi.customers.get(rest[0]);
          print(c, () => describeCustomer(c));
          return 0;
        }
        case "list": {
          const list = await oi.customers.list(defined({ external_id: values["external-id"], ...page() }));
          printList(list.data, describeCustomer, "No customers.");
          return 0;
        }
        case "portal": {
          if (!rest[0]) throw new Error("Usage: openintents customers portal <id>");
          const p = await oi.customers.createPortalSession(
            rest[0],
            defined({ return_url: values["return-url"], ...country(), ttl_seconds: values.ttl ? int("--ttl", values.ttl) : undefined }),
          );
          print(p, () => describePortal(p));
          return 0;
        }
        default:
          throw new Error("Usage: openintents customers create|update|get|list|portal");
      }
    }
    case "sessions": {
      const oi = await io.client();
      switch (sub) {
        case "open": {
          const s = await oi.sessions.create(
            defined({
              profile: values.profile,
              customer: values.customer,
              start_url: values["start-url"],
              max_minutes: values["max-minutes"] ? int("--max-minutes", values["max-minutes"]) : undefined,
              ...country(),
              webhook_url: values["webhook-url"],
            }),
          );
          print(s, () => describeSession(s));
          return 0;
        }
        case "list": {
          const list = await oi.sessions.list(
            defined({ status: values.status as SessionStatus | undefined, profile: values.profile, customer: values.customer, ...page() }),
          );
          printList(list.data, describeSession, "No sessions.");
          return 0;
        }
        case "get":
        case "release": {
          if (!rest[0]) throw new Error(`Usage: openintents sessions ${sub} <id>`);
          const s = sub === "get" ? await oi.sessions.get(rest[0]) : await oi.sessions.release(rest[0]);
          print(s, () => describeSession(s));
          return 0;
        }
        case "live": {
          if (!rest[0]) throw new Error("Usage: openintents sessions live <id>");
          const link = await oi.sessions.createLiveLink(rest[0]);
          print(link, () => describeLiveLink(link));
          return 0;
        }
        case "command": {
          const [sid, ...words] = rest;
          if (!sid) throw new Error("Usage: openintents sessions command <id> <command> [args]");
          const result = await oi.sessions.runCommand(sid, sessionCommand(words));
          print(result, () => describeCommand(result));
          return result.status === "refused" || result.status === "failed" ? 1 : 0;
        }
        case "screenshot": {
          if (!rest[0] || !values.out) throw new Error("Usage: openintents sessions screenshot <id> --out <file.jpg>");
          const jpeg = await oi.sessions.screenshot(rest[0]);
          await writeFile(values.out, jpeg);
          print({ session: rest[0], file: values.out, bytes: jpeg.length }, () => `Saved ${jpeg.length} bytes to ${values.out}`);
          return 0;
        }
        default:
          throw new Error("Usage: openintents sessions open|list|get|release|live|command|screenshot");
      }
    }
    case "profiles": {
      const oi = await io.client();
      switch (sub) {
        case "list": {
          const list = await oi.profiles.list(defined({ customer: values.customer, ...page() }));
          printList(list.data, describeProfile, "No profiles.");
          return 0;
        }
        case "create": {
          const p = await oi.profiles.create(defined({ customer: values.customer }));
          print(p, () => describeProfile(p));
          return 0;
        }
        case "get":
        case "forget": {
          if (!rest[0]) throw new Error(`Usage: openintents profiles ${sub} <id>`);
          const p = sub === "get" ? await oi.profiles.get(rest[0]) : await oi.profiles.forget(rest[0]);
          print(p, () => describeProfile(p));
          return 0;
        }
        default:
          throw new Error("Usage: openintents profiles list|create|get|forget");
      }
    }
    case "sign-ins": {
      const oi = await io.client();
      switch (sub) {
        case "create": {
          if (!rest[0]) throw new Error("Usage: openintents sign-ins create <shop url>");
          const s = await oi.signIns.create(
            defined({ shop_url: rest[0], customer: values.customer, ...country(), return_url: values["return-url"] }),
          );
          print(s, () => describeSignIn(s));
          return 0;
        }
        case "get": {
          if (!rest[0]) throw new Error("Usage: openintents sign-ins get <id>");
          const s = await oi.signIns.get(rest[0]);
          print(s, () => describeSignIn(s));
          return 0;
        }
        default:
          throw new Error("Usage: openintents sign-ins create|get");
      }
    }
    case "secrets": {
      const oi = await io.client();
      switch (sub) {
        case "set": {
          const [kind] = values.kind ?? [];
          if (!kind || !values.label) throw new Error("Usage: openintents secrets set --kind <kind> --label <name> (--field name=value... | --stdin)");
          const fields = {
            ...(values.stdin ? parseFields(await io.stdin()) : {}),
            ...pairs("--field", values.field),
          };
          if (!Object.keys(fields).length) throw new Error("A secret needs fields: pass --field name=value or --stdin.");
          const s = await oi.secrets.create(
            defined({
              customer: values.customer,
              kind: kind as SecretKind,
              label: values.label,
              fields,
              shops: values.shop?.length ? values.shop : undefined,
              default: values.default || undefined,
              expires_at: values["expires-at"],
            }),
          );
          print(s, () => describeSecret(s));
          return 0;
        }
        case "list": {
          const list = await oi.secrets.list(defined({ customer: values.customer, kind: values.kind?.[0] as SecretKind | undefined }));
          printList(list.data, describeSecret, "No secrets.");
          return 0;
        }
        case "get": {
          if (!rest[0]) throw new Error("Usage: openintents secrets get <id>");
          const s = await oi.secrets.get(rest[0]);
          print(s, () => describeSecret(s));
          return 0;
        }
        case "delete": {
          if (!rest[0]) throw new Error("Usage: openintents secrets delete <id>");
          await oi.secrets.delete(rest[0]);
          print({ id: rest[0], deleted: true }, () => `Deleted ${rest[0]}`);
          return 0;
        }
        case "request": {
          if (!rest[0] || !values.kind?.length) throw new Error("Usage: openintents secrets request <customer id> --kind <kind>...");
          const r = await oi.customers.createSecretRequest(
            rest[0],
            defined({ kinds: values.kind as SecretKind[], return_url: values["return-url"], ttl_seconds: values.ttl ? int("--ttl", values.ttl) : undefined }),
          );
          print(r, () => describeSecretRequest(r));
          return 0;
        }
        default:
          throw new Error("Usage: openintents secrets set|list|get|delete|request");
      }
    }
    case "mcp": {
      // stdout belongs to the protocol: nothing else may be printed on it.
      const key = await (io.apiKey ?? (async () => { throw new Error("No API key."); }))();
      await runMcpBridge({ url: values.url ?? mcpUrl(), apiKey: key });
      return 0;
    }
    case "login": {
      io.out(`Create a key at ${DASHBOARD_KEYS_URL} (opening it now).`);
      io.openUrl(DASHBOARD_KEYS_URL);
      io.out("Paste your API key:");
      const key = (await io.stdin()).trim();
      if (!/^oi_(live|test)_/.test(key)) throw new Error("That doesn't look like an OpenIntents key (oi_live_... or oi_test_...).");
      await mkdir(dirname(io.configFile), { recursive: true, mode: 0o700 });
      await writeFile(io.configFile, JSON.stringify({ apiKey: key }, null, 2), { mode: 0o600 });
      io.out(`Saved to ${io.configFile}.`);
      return 0;
    }
    default:
      throw new Error(`Unknown command "${command}". Run \`openintents --help\`.`);
  }
}

/** The fields of a secret as a JSON object of strings. Errors never quote the input. */
function parseFields(text: string): Record<string, string> {
  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch {
    throw new Error("--stdin expects a JSON object of field names to text values.");
  }
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed) || Object.values(parsed).some((v) => typeof v !== "string")) {
    throw new Error("--stdin expects a JSON object of field names to text values.");
  }
  return parsed as Record<string, string>;
}

/** Reads the key `openintents login` saved. */
export async function savedKey(configFile: string): Promise<string | undefined> {
  try {
    return (JSON.parse(await readFile(configFile, "utf8")) as { apiKey?: string }).apiKey;
  } catch {
    return undefined;
  }
}
