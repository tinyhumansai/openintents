# Changelog

All three SDKs share one version. Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

## [0.1.1]

### Changed

- New tagline everywhere: "Let your agents buy anything." Package descriptions,
  READMEs, CLI help and the spec summary lead with the outcome.
- Every package ships its LICENSE; the npm CLI entry is declared in the form
  npm 11 accepts.

## [0.1.0]

### Added

- OpenAPI spec for the v1 preview API (`spec/openapi.yaml`).
- TypeScript SDK and `openintents` CLI (`@tinyhumansai/openintents`).
- Python SDK with sync and async clients (`openintents`).
- Rust SDK with an optional CLI (`openintents`).
- Webhook signature verification in every SDK, with a shared test vector.
- Agent skill and Claude Code plugin.
