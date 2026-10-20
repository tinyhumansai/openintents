#!/usr/bin/env node
// Keeps every SDK on one version.
//   node sdk/bump-versions.mjs <patch|minor|major|current|x.y.z> [--dry-run]
//       bumps every file below together and prints the new version
//       (`current` changes nothing and prints the version they share).
//   node sdk/bump-versions.mjs --check [x.y.z]
//       prints "<file> <version>" for each file; exits 1 if they disagree
//       (or with x.y.z, if they are not all x.y.z).
// Files: TS package.json + src/version.ts, Python pyproject.toml + _version.py
// + uv.lock, Rust Cargo.toml + Cargo.lock, the skill and the Claude plugin.
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);
const check = args.includes("--check");
const dryRun = args.includes("--dry-run");
const [arg] = args.filter((a) => !a.startsWith("--"));

// [file, pattern: group 1 is what comes before the version, group 2 the version, group 3 what follows]
const files = [
  ["typescript/package.json", /("version":\s*")([^"]+)(")/],
  ["typescript/src/version.ts", /(VERSION = ")([^"]+)(")/],
  ["python/pyproject.toml", /(^version = ")([^"]+)(")/m],
  ["python/src/openintents/_version.py", /(__version__ = ")([^"]+)(")/],
  ["python/uv.lock", /(\nname = "openintents"\nversion = ")([^"]+)(")/],
  ["rust/Cargo.toml", /(\[package\][\s\S]*?\nversion = ")([^"]+)(")/],
  ["rust/Cargo.lock", /(\nname = "openintents"\nversion = ")([^"]+)(")/],
  ["skill/openintents/SKILL.md", /(\n  version: ")([^"]+)(")/],
  ["plugin-claude/.claude-plugin/plugin.json", /("version":\s*")([^"]+)(")/],
];

const read = (file) => readFileSync(join(root, file), "utf8");

function versionOf(file, re) {
  const m = re.exec(read(file));
  if (!m) throw new Error(`No version found in ${file}`);
  return m[2];
}

if (check) {
  const found = files.map(([file, re]) => [file, versionOf(file, re)]);
  for (const [file, version] of found) console.log(`${file} ${version}`);
  const want = arg ?? found[0][1];
  const off = found.filter(([, version]) => version !== want);
  if (off.length) {
    console.error(`Expected every file at ${want}, but:\n${off.map(([f, v]) => `  ${f} is ${v}`).join("\n")}`);
    process.exit(1);
  }
  process.exit(0);
}

const current = versionOf(...files[0]);
const next = (() => {
  if (/^\d+\.\d+\.\d+$/.test(arg ?? "")) return arg;
  const [maj, min, pat] = current.split(".").map(Number);
  if (arg === "current") return current;
  if (arg === "major") return `${maj + 1}.0.0`;
  if (arg === "minor") return `${maj}.${min + 1}.0`;
  if (arg === "patch") return `${maj}.${min}.${pat + 1}`;
  console.error("usage: bump-versions.mjs <patch|minor|major|current|x.y.z> [--dry-run] | --check [x.y.z]");
  process.exit(1);
})();

for (const [file, re] of files) {
  const before = read(file);
  if (!re.test(before)) throw new Error(`No version found in ${file}`);
  if (!dryRun) writeFileSync(join(root, file), before.replace(re, `$1${next}$3`));
}
console.log(next);
