#!/usr/bin/env node
// Bump every SDK version together: node sdk/bump-versions.mjs <patch|minor|major|x.y.z> [--dry-run]
// Prints the new version. Files: TS package.json + src/version.ts, Python
// pyproject.toml + _version.py, Rust Cargo.toml, the skill and the Claude plugin.
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(fileURLToPath(import.meta.url));
const [arg, ...flags] = process.argv.slice(2);
const dryRun = flags.includes("--dry-run");

const current = JSON.parse(readFileSync(join(root, "typescript/package.json"), "utf8")).version;
const next = (() => {
  if (/^\d+\.\d+\.\d+$/.test(arg ?? "")) return arg;
  const [maj, min, pat] = current.split(".").map(Number);
  if (arg === "major") return `${maj + 1}.0.0`;
  if (arg === "minor") return `${maj}.${min + 1}.0`;
  if (arg === "patch") return `${maj}.${min}.${pat + 1}`;
  console.error("usage: bump-versions.mjs <patch|minor|major|x.y.z> [--dry-run]");
  process.exit(1);
})();

// [file, pattern capturing the version, replacement]
const edits = [
  ["typescript/package.json", /("version":\s*")[^"]+(")/],
  ["typescript/src/version.ts", /(VERSION = ")[^"]+(")/],
  ["python/pyproject.toml", /(^version = ")[^"]+(")/m],
  ["python/src/openintents/_version.py", /(__version__ = ")[^"]+(")/],
  ["rust/Cargo.toml", /(\[package\][\s\S]*?\nversion = ")[^"]+(")/],
  ["skill/openintents/SKILL.md", /(\n  version: ")[^"]+(")/],
  ["plugin-claude/.claude-plugin/plugin.json", /("version":\s*")[^"]+(")/],
];

for (const [file, re] of edits) {
  const path = join(root, file);
  const before = readFileSync(path, "utf8");
  if (!re.test(before)) throw new Error(`No version found in ${file}`);
  const after = before.replace(re, `$1${next}$2`);
  if (!dryRun) writeFileSync(path, after);
}
console.log(next);
