#!/usr/bin/env node
import { homedir } from "node:os";
import { join } from "node:path";
import { createInterface } from "node:readline/promises";
import { spawn } from "node:child_process";

import { OpenIntents } from "./client.js";
import { runCli, savedKey } from "./cli-main.js";

const CONFIG_DIR = join(process.env.XDG_CONFIG_HOME ?? join(homedir(), ".config"), "openintents");
const CONFIG_FILE = join(CONFIG_DIR, "config.json");

async function apiKey(): Promise<string> {
  const key = process.env.OPENINTENTS_API_KEY ?? (await savedKey(CONFIG_FILE));
  if (!key) throw new Error("No API key. Run `openintents login` or set OPENINTENTS_API_KEY.");
  return key;
}

function openUrl(url: string) {
  const cmd = process.platform === "darwin" ? "open" : process.platform === "win32" ? "cmd" : "xdg-open";
  const args = process.platform === "win32" ? ["/c", "start", "", url] : [url];
  spawn(cmd, args, { stdio: "ignore", detached: true }).unref();
}

/** One line on a terminal; everything piped in otherwise. */
async function stdin(): Promise<string> {
  if (process.stdin.isTTY) {
    const rl = createInterface({ input: process.stdin, output: process.stdout });
    const line = await rl.question("");
    rl.close();
    return line;
  }
  let text = "";
  for await (const chunk of process.stdin) text += chunk;
  return text;
}

runCli(process.argv.slice(2), {
  client: async () => new OpenIntents({ apiKey: await apiKey() }),
  apiKey,
  out: (line) => console.log(line),
  err: (line) => console.error(line),
  stdin,
  openUrl,
  configFile: CONFIG_FILE,
}).then((code) => process.exit(code));
