# Security policy

## Reporting a vulnerability

Please **do not open a public issue** for security problems. Report them
privately through GitHub's
[private vulnerability reporting](https://github.com/tinyhumansai/openintents/security/advisories/new)
for this repository.

Include what you found, how to reproduce it, and the impact you expect. We aim
to acknowledge reports within three business days and will keep you updated
until it's resolved. We're happy to credit you once a fix ships.

## Scope

This repository contains the client SDKs, CLI, spec and plugins. Issues in the
hosted API or website can be reported the same way.

## Handling keys

- Never commit API keys (`oi_live_...`, `oi_test_...`) or webhook secrets
  (`whsec_...`). If one leaks, revoke it in the dashboard immediately.
- The CLI stores a key saved with `openintents login` in
  `~/.config/openintents/config.json` with `0600` permissions.
- Always verify webhook signatures (`verifyWebhook` / `verify_webhook`) before
  trusting a payload.
