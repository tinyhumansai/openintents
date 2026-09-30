#!/usr/bin/env node
/**
 * The `openintents` command: a thin client for the REST API, for
 * shell-driven agents and for people trying intents out.
 */
import { spawn } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { homedir } from "node:os";
import { join } from "node:path";
import { createInterface } from "node:readline/promises";
import { parseArgs } from "node:util";

import { OpenIntents } from "./client.js";
import { OpenIntentsError, WaitTimeoutError } from "./errors.js";
import type { Intent, IntentStatus } from "./types.js";
import { VERSION } from "./version.js";

const DASHBOARD_KEYS_URL = "https://openintents.io/dashboard/api-keys";
const CONFIG_DIR = join(process.env.XDG_CONFIG_HOME ?? join(homedir(), ".config"), "openintents");
const CONFIG_FILE = join(CONFIG_DIR, "config.json");

/** Exit codes, as documented: scripts can branch on the outcome. */
const EXIT: Record<IntentStatus | "error", number> = {
  completed: 0,
  queued: 0,
  running: 0,
  failed: 1,
  error: 1,
  requires_payment: 2,
  cancelled: 3,
};

const HELP = `openintents ${VERSION}: the agentic API to buy anything

Usage:
  openintents run "<intent>" [--max <dollars>] [--context key=value]... [--payment-link] [--wait]
  openintents status <id> [--wait]
  openintents list [--status <status>] [--limit <n>]
  openintents cancel <id>
  openintents pay <id>          open the intent's payment link
  openintents balance
  openintents login             save an API key to ${CONFIG_FILE}

Every command takes --json for one JSON object per line.
Auth: OPENINTENTS_API_KEY, or the key saved by \`openintents login\`.
Exit codes: 0 completed, 1 failed or error, 2 needs payment, 3 cancelled.`;

async function savedKey(): Promise<string | undefined> {
  try {
    return (JSON.parse(await readFile(CONFIG_FILE, "utf8")) as { apiKey?: string }).apiKey;
  } catch {
    return undefined;
  }
}

async function client(): Promise<OpenIntents> {
  const apiKey = process.env.OPENINTENTS_API_KEY ?? (await savedKey());
  if (!apiKey) {
    throw new Error("No API key. Run `openintents login` or set OPENINTENTS_API_KEY.");
  }
  return new OpenIntents({ apiKey });
}

function openUrl(url: string) {
  const cmd = process.platform === "darwin" ? "open" : process.platform === "win32" ? "cmd" : "xdg-open";
  const args = process.platform === "win32" ? ["/c", "start", "", url] : [url];
  spawn(cmd, args, { stdio: "ignore", detached: true }).unref();
}

const money = (cents: number, currency = "usd") =>
  new Intl.NumberFormat("en-US", { style: "currency", currency: currency.toUpperCase() }).format(cents / 100);

function describe(intent: Intent): string {
  const lines = [`${intent.id}  ${intent.status}  "${intent.intent}"`];
  if (intent.payment) {
    lines.push(`  payment needed: ${money(intent.payment.total, intent.payment.currency)} (+${money(intent.payment.fee, intent.payment.currency)} fee)`);
    lines.push(`  pay here: ${intent.payment.payment_url}`);
  }
  if (intent.result) {
    const r = intent.result;
    lines.push(`  ${r.merchant}: ${money(r.total, r.currency)} (+${money(r.fee, r.currency)} fee)${r.confirmation ? `, confirmation ${r.confirmation}` : ""}`);
  }
  if (intent.error) lines.push(`  error ${intent.error.code}: ${intent.error.message}`);
  return lines.join("\n");
}

function print(value: unknown, json: boolean, human: () => string) {
  console.log(json ? JSON.stringify(value) : human());
}

async function main(argv: string[]): Promise<number> {
  const { values, positionals } = parseArgs({
    args: argv,
    allowPositionals: true,
    options: {
      json: { type: "boolean", default: false },
      wait: { type: "boolean", default: false },
      max: { type: "string" },
      context: { type: "string", multiple: true },
      "payment-link": { type: "boolean", default: false },
      status: { type: "string" },
      limit: { type: "string" },
      help: { type: "boolean", short: "h", default: false },
      version: { type: "boolean", short: "v", default: false },
    },
  });
  const [command, arg] = positionals;
  const json = values.json ?? false;

  if (values.version) {
    console.log(VERSION);
    return 0;
  }
  if (values.help || !command) {
    console.log(HELP);
    return command || values.help ? 0 : 1;
  }

  switch (command) {
    case "run": {
      if (!arg) throw new Error('Usage: openintents run "<intent>"');
      const context = Object.fromEntries(
        (values.context ?? []).map((kv) => {
          const i = kv.indexOf("=");
          if (i < 1) throw new Error(`--context expects key=value, got "${kv}"`);
          return [kv.slice(0, i), kv.slice(i + 1)];
        }),
      );
      const oi = await client();
      let intent = await oi.intents.create({
        intent: arg,
        max_amount: values.max ? Math.round(Number(values.max) * 100) : undefined,
        context: Object.keys(context).length ? context : undefined,
        require_payment_link: values["payment-link"] || undefined,
      });
      if (values.wait) intent = await oi.intents.wait(intent.id);
      print(intent, json, () => describe(intent));
      return EXIT[intent.status];
    }
    case "status": {
      if (!arg) throw new Error("Usage: openintents status <id>");
      const oi = await client();
      const intent = values.wait ? await oi.intents.wait(arg) : await oi.intents.get(arg);
      print(intent, json, () => describe(intent));
      return EXIT[intent.status];
    }
    case "list": {
      const oi = await client();
      const page = await oi.intents.list({
        status: values.status as IntentStatus | undefined,
        limit: values.limit ? Number(values.limit) : undefined,
      });
      if (json) page.data.forEach((i) => console.log(JSON.stringify(i)));
      else console.log(page.data.map(describe).join("\n") || "No intents.");
      return 0;
    }
    case "cancel": {
      if (!arg) throw new Error("Usage: openintents cancel <id>");
      const intent = await (await client()).intents.cancel(arg);
      print(intent, json, () => describe(intent));
      return EXIT[intent.status];
    }
    case "pay": {
      if (!arg) throw new Error("Usage: openintents pay <id>");
      const intent = await (await client()).intents.get(arg);
      if (intent.status !== "requires_payment" || !intent.payment) {
        throw new Error(`${intent.id} doesn't need payment (status: ${intent.status}).`);
      }
      if (!json) openUrl(intent.payment.payment_url);
      print(intent.payment, json, () => `Opened ${intent.payment!.payment_url}`);
      return 2;
    }
    case "balance": {
      const b = await (await client()).balance.get();
      print(b, json, () => {
        const spent = b.spent_this_month !== undefined ? `, ${money(b.spent_this_month, b.currency)} spent this month` : "";
        return `${money(b.available, b.currency)} available${spent}`;
      });
      return 0;
    }
    case "login": {
      console.log(`Create a key at ${DASHBOARD_KEYS_URL} (opening it now).`);
      openUrl(DASHBOARD_KEYS_URL);
      const rl = createInterface({ input: process.stdin, output: process.stdout });
      const key = (await rl.question("Paste your API key: ")).trim();
      rl.close();
      if (!/^oi_(live|test)_/.test(key)) throw new Error("That doesn't look like an OpenIntents key (oi_live_... or oi_test_...).");
      await mkdir(CONFIG_DIR, { recursive: true, mode: 0o700 });
      await writeFile(CONFIG_FILE, JSON.stringify({ apiKey: key }, null, 2), { mode: 0o600 });
      console.log(`Saved to ${CONFIG_FILE}.`);
      return 0;
    }
    default:
      throw new Error(`Unknown command "${command}". Run \`openintents --help\`.`);
  }
}

main(process.argv.slice(2)).then(
  (code) => process.exit(code),
  (err: unknown) => {
    const json = process.argv.includes("--json");
    const message =
      err instanceof OpenIntentsError
        ? `${err.message} (${err.type}${err.code ? `/${err.code}` : ""})`
        : err instanceof WaitTimeoutError
          ? err.message
          : (err as Error).message;
    if (json) console.log(JSON.stringify({ error: { message } }));
    else console.error(`openintents: ${message}`);
    process.exit(EXIT.error);
  },
);
