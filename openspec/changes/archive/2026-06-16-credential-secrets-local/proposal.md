## Why

Local setup still relies on a flat `.env` with many `AI_GATEWAY_CREDENTIAL_*` vars
plus ad-hoc `CHATGPT_BROWSER_CLI` / `HELICONE_*` overrides. Policy (tier,
budget-rank, cost-class) is already in embedded `credentials.yaml` — secrets
should live in **one gitignored YAML**, not scattered env names.

Backward compatibility with legacy env aliases is **not a goal** for this change.

Release target: **`0.3.0-beta.18`** (after `autodefault-routing-priority` beta.17).

## What Changes

- **Single secrets file** (`dev/secrets.local.yaml`) for provider keys, browser
  session paths, Helicone API key, and optional AWS/Bedrock keys.
- Discovery via `AI_GATEWAY_SECRETS_FILE` (or default paths); **no per-slot env
  vars** in the documented workflow.
- **Remove** resolution of `AI_GATEWAY_CREDENTIAL_*`, `{PROVIDER}_API_KEY`,
  `GEMINI_FREE_TIER_*`, `CHATGPT_BROWSER_CLI`, `DEEPSEEK_BROWSER_CLI`,
  `HELICONE_CONTROL_PLANE_API_KEY`, and `AWS_*` env overrides.
- Non-secret settings (Helicone URLs, telemetry level, routers) stay in
  `config/local.yaml` / `AI_GATEWAY__*` — not in the secrets file.
- Drop `.env` from the primary dev workflow; optional `dotenvy` load removed or
  limited to `AI_GATEWAY_SECRETS_FILE` pointer only.

## Capabilities

### New Capabilities

- `credential-secrets-local`: Unified local secrets file, loader, and removal of
  legacy credential env resolution.

### Modified Capabilities

- None.

## Impact

- `credentials.rs`, `credential_env.rs`, new `secrets_file.rs`
- Remove env overrides in `read.rs` (Helicone, AWS region)
- `helicone/mod.rs` — stop reading `HELICONE_CONTROL_PLANE_API_KEY` at default
- `dev/secrets.local.example.yaml`, `.gitignore`, docs
- **Breaking** for anyone using old env var names
