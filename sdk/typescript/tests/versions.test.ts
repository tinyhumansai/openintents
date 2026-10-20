import { spawnSync } from "node:child_process";
import { cp, mkdir, mkdtemp, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

import { VERSION } from "../src/version.js";

const sdk = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const script = join(sdk, "bump-versions.mjs");

/** Runs the script (a copy, if `dir` is given) and returns what it printed. */
function run(args: string[], dir = sdk) {
  const r = spawnSync("node", [join(dir, "bump-versions.mjs"), ...args], { encoding: "utf8" });
  return { code: r.status, out: r.stdout, err: r.stderr };
}

/** A copy of the script and every file it manages, to change freely. */
async function copyOfTree(): Promise<string> {
  const dir = await mkdtemp(join(tmpdir(), "oi-versions-"));
  await cp(script, join(dir, "bump-versions.mjs"));
  const listed = run(["--check"]).out.trim().split("\n");
  for (const line of listed) {
    const file = line.split(/\s+/)[0]!;
    await mkdir(dirname(join(dir, file)), { recursive: true });
    await cp(join(sdk, file), join(dir, file));
  }
  return dir;
}

describe("one version everywhere", () => {
  it("has every manifest, lock file, skill and plugin at the SDK's version", () => {
    const r = run(["--check"]);
    expect(r.err).toBe("");
    expect(r.code).toBe(0);
    const files = r.out.trim().split("\n").map((l) => l.split(/\s+/));
    expect(files.map(([f]) => f)).toEqual(
      expect.arrayContaining([
        "typescript/package.json",
        "typescript/src/version.ts",
        "python/pyproject.toml",
        "python/src/openintents/_version.py",
        "python/uv.lock",
        "rust/Cargo.toml",
        "rust/Cargo.lock",
        "skill/openintents/SKILL.md",
        "plugin-claude/.claude-plugin/plugin.json",
      ]),
    );
    for (const [, version] of files) expect(version).toBe(VERSION);
  });

  it("checks against the version a release is about to publish", () => {
    expect(run(["--check", VERSION]).code).toBe(0);
    const wrong = run(["--check", "9.9.9"]);
    expect(wrong.code).toBe(1);
    expect(wrong.err).toContain("9.9.9");
  });

  it("fails and names the file when one disagrees", async () => {
    const dir = await copyOfTree();
    const file = join(dir, "python/src/openintents/_version.py");
    await writeFile(file, (await readFile(file, "utf8")).replace(VERSION, "0.0.1"));
    const r = run(["--check"], dir);
    expect(r.code).toBe(1);
    expect(r.err).toContain("python/src/openintents/_version.py");
  });

  it("bumps all of them together, lock files included", async () => {
    const dir = await copyOfTree();
    expect(run(["0.9.5"], dir).out.trim()).toBe("0.9.5");
    const r = run(["--check", "0.9.5"], dir);
    expect(r.code).toBe(0);
    expect(r.out).toContain("rust/Cargo.lock");
    expect(await readFile(join(dir, "python/uv.lock"), "utf8")).toContain('name = "openintents"\nversion = "0.9.5"');
  });
});
