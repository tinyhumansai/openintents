# AGENTS.md

Guidance for coding agents (Claude Code, Codex, …) and contributors working in
this repository. `CLAUDE.md` is a symlink to this file: edit `AGENTS.md`.

> If a `CLAUDE.local.md` exists at the repo root, read it first. It holds
> personal, git-ignored notes that aren't part of this shared config.

## What this repo is

`tinyhumansai/openintents` is the **public, open-source** client side of
OpenIntents, the agentic API to buy anything: SDKs (TypeScript, Python, Rust),
the `openintents` CLI, the OpenAPI spec, an agent skill and a Claude Code
plugin. The hosted API, website and dashboard are **not** in this repo.

## This repository is public

Everything committed here is world-readable and permanent (forks and caches
keep it even after a revert). Before committing, make sure nothing private
comes along:

- **No secrets:** API keys (`oi_live_`/`oi_test_` values), webhook secrets
  (`whsec_`), tokens, `.env` files. Use obvious placeholders (`oi_test_...`).
- **No internal infrastructure:** hostnames other than the public
  `openintents.io` / `api.openintents.io`, cloud account or cluster details,
  deployment manifests, internal service names, dashboards or runbooks.
- **No references to private repositories or their paths**, internal tickets,
  customer data, or unreleased business details (pricing experiments,
  partners, roadmaps not already on the website).
- **Only public API behaviour:** describe what `spec/openapi.yaml` and the
  public docs say, not how the service implements it.

If a change would need private context to explain, it belongs somewhere else.

## Layout

| Path | Package | Toolchain |
| --- | --- | --- |
| `spec/openapi.yaml` | | The v1 contract. Change it first, then the SDKs. |
| `sdk/typescript/` | `@tinyhumansai/openintents` (SDK + `openintents` bin) | Node 20.3+, pnpm 10, TypeScript, Vitest. Zero runtime deps. |
| `sdk/python/` | `openintents` | Python 3.10+, httpx, uv, pytest, ruff |
| `sdk/rust/` | `openintents` (CLI behind `--features cli`) | Rust stable, reqwest (rustls), wiremock |
| `sdk/skill/openintents/` | Agent skill; `SKILL.md` at the root links here | |
| `sdk/plugin-claude/` | Claude Code plugin; `.claude-plugin/marketplace.json` lists it | |
| `sdk/examples/` | One runnable example per language | |

pnpm workspace (`pnpm-workspace.yaml`) covers `sdk/typescript` only.

## Commands (what CI runs)

```bash
# TypeScript
pnpm install
pnpm --filter @tinyhumansai/openintents lint     # tsc --noEmit
pnpm --filter @tinyhumansai/openintents test     # vitest
pnpm --filter @tinyhumansai/openintents build

# Python (from sdk/python)
uv run --extra dev ruff check src tests
uv run --extra dev ruff format --check src tests
uv run --extra dev pytest

# Rust (from sdk/rust)
cargo fmt --check
cargo clippy --all-targets --features cli -- -D warnings
cargo test
```

## Rules for SDK changes

- **Keep the three SDKs in parity.** Same operations, same defaults
  (`wait` returns on `requires_payment` unless told not to; 2 retries on
  429/5xx/network with backoff; automatic `Idempotency-Key` on create and
  cancel), same error fields. Update the parity table in `sdk/README.md`.
- **Spec first.** New fields or endpoints go in `spec/openapi.yaml`, then all
  SDKs, then the READMEs.
- **Be tolerant of new API data.** Unknown fields must not break parsing; Rust
  maps unknown statuses to `IntentStatus::Unknown`.
- **Webhook signatures are shared.** `OpenIntents-Signature: t=<unix>,v1=<hex>`,
  HMAC-SHA256 of `"{t}.{raw body}"`, 300s tolerance, constant-time compare.
  Each suite asserts the same test vector; if you change signing, change all
  three.
- **Money is integer cents** everywhere; never floats in the API layer (the
  CLI's `--max` takes dollars and converts).
- **No new runtime dependencies** in the TypeScript SDK; keep Python to httpx.
- **Versions move together:** `node sdk/bump-versions.mjs <patch|minor|major>`.
  Don't hand-edit versions.
- **Tests with every change**, using mocked HTTP (Vitest mock fetch,
  `httpx.MockTransport`, wiremock). No test may call the real API.

## Style

- Plain, direct prose in docs and comments. **No em or en dashes** in any
  written copy; use a comma, colon or full stop.
- Comments explain why, not what. Public items get doc comments.
- Commit messages: conventional prefix (`feat(ts):`, `fix(python):`,
  `docs:`), small focused commits, no squashing of others' history.

## Releasing

Manual, via the **Release** workflow (`workflow_dispatch`): pick the SDKs and
the bump. It bumps all versions, commits to `main`, tags `vX.Y.Z` and
publishes the selected packages (npm with provenance, PyPI, crates.io).
Secrets: `NPM_TOKEN` and `CARGO_REGISTRY_TOKEN` (repository), and
`PYPI_API_TOKEN` on the `Production` environment (deployable from `main`
only).
