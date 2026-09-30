# Contributing

Thanks for helping! OpenIntents' SDKs are small on purpose: a thin, typed
client per language that follows [`spec/openapi.yaml`](spec/openapi.yaml).

1. **Open an issue first** for anything bigger than a bug fix, so we can agree
   on the shape (and keep the three SDKs in parity).
2. Fork, branch, and make your change with tests. Run the checks for every SDK
   you touched (see [AGENTS.md](AGENTS.md#commands-what-ci-runs)).
3. Open a pull request describing what changed and why. CI must pass.

Guidelines:

- Spec first: API shape changes start in `spec/openapi.yaml`.
- Keep TypeScript, Python and Rust in parity and update `sdk/README.md`.
- Tests use mocked HTTP only; never call the real API.
- Don't include secrets or private information; this repo is public.

By contributing you agree that your contributions are licensed under the
[GPL-3.0-or-later](LICENSE), and you agree to follow our
[Code of Conduct](CODE_OF_CONDUCT.md).
