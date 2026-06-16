## Context

Two concerns were mixed in `.env`:

1. **Secrets** — API keys, session files, Helicone key, AWS keys
2. **Config** — server port, telemetry level, Helicone base URLs (already in `local.yaml`)

Provider **policy** is in embedded `credentials.yaml`. Operators should not need
15+ env vars or a second convention for Helicone.

## Goals / Non-Goals

**Goals:**

- One gitignored **`secrets.local.yaml`** for all sensitive values.
- One optional env pointer: **`AI_GATEWAY_SECRETS_FILE`** (path to that file).
- K8s: mount the same YAML via Secret volume + `AI_GATEWAY_SECRETS_FILE`.
- Non-secret config in `config/local.yaml` (or `-c` file) + existing `AI_GATEWAY__*`
  merge for deployment tuning only.

**Non-Goals:**

- Backward compatibility with `AI_GATEWAY_CREDENTIAL_*`, `OPENAI_API_KEY`,
  `CHATGPT_BROWSER_CLI`, `HELICONE_CONTROL_PLANE_API_KEY`, etc.
- Keeping `.env` as the documented dev entrypoint.
- Putting non-secrets (Helicone URLs, `OTEL_*`) in the secrets file — those belong
  in config YAML or platform env for the collector, not next to API keys.

## Decisions

### D1 — Two files, two roles

| File | Git | Contents |
| --- | --- | --- |
| `config/local.yaml` | commit OK | `telemetry`, `helicone.base-url`, `routers`, server |
| `dev/secrets.local.yaml` | **gitignore** | keys, session paths, integration secrets |

No `.env` in the happy path.

### D2 — Secrets file schema (v1)

Discovery (first existing):

1. `AI_GATEWAY_SECRETS_FILE`
2. `./dev/secrets.local.yaml`
3. `~/.config/ai-gateway/secrets.yaml`

```yaml
# dev/secrets.local.yaml
credentials:
  openrouter-default:
    api-key: sk-or-...
  gemini-free:
    api-key: ...
  gemini-free-2:
    api-key: ...
  cloudflare-default:
    api-key: "account_id:cfut_..."
  github-models-default:
    api-key: ghp_...
  deepseek-web-default:
    session-file: dev/deepseek-session.json
  chatgpt-web-default:
    session-file: dev/session.json

integrations:
  helicone:
    api-key: sk-helicone-...
  aws:                          # optional — Bedrock
    access-key: ...
    secret-key: ...
    region: eu-central-1
```

Per credential slot fields: `api-key` | `api-key-file` | `session-file`.

Relative paths resolve from the secrets file directory.

Committed: `dev/secrets.local.example.yaml` (placeholders only).

### D3 — What moves out of env permanently

**Removed from gateway credential resolution (breaking):**

- `AI_GATEWAY_CREDENTIAL_<ID>`
- `{PROVIDER}_API_KEY` legacy
- `GEMINI_FREE_TIER_API_KEY` / `GEMINI_FREE_TIER_APIKEY`
- `CLOUDFLARE_API_KEY_WITH_ACCOUNT_ID` / split Cloudflare env
- `CHATGPT_BROWSER_CLI`, `DEEPSEEK_BROWSER_CLI`, `PERPLEXITY_BROWSER_CLI`

**Removed special-case env overrides in `Config::try_read`:**

- `HELICONE_CONTROL_PLANE_API_KEY` → `integrations.helicone.api-key` in secrets file
  (or `helicone.api-key` in config if operator accepts key in config — prefer secrets file)
- `AWS_REGION` / `AWS_ACCESS_KEY` / `AWS_SECRET_KEY` → `integrations.aws` in secrets file

### D4 — Helicone and OTEL

**Helicone:** URL/features stay in `config/local.yaml` (already there). API key
only in `secrets.local.yaml` under `integrations.helicone.api-key`. Remove
`default_api_key()` reading env; loader applies secrets after config merge.

**OTEL:** `OTEL_METRIC_EXPORT_INTERVAL` in `.env.template` is deployment/fly
plumbing for the collector, not gateway credential config. Gateway telemetry
level/export already lives under `telemetry:` in YAML. Do **not** document a
separate `.env` block for OTEL in ai-gateway dev setup — use `config/local.yaml`
or platform manifests.

### D5 — Production (K8s)

Single Secret mounted as file:

```yaml
env:
  - name: AI_GATEWAY_SECRETS_FILE
    value: /etc/ai-gateway/secrets.yaml
volumeMounts:
  - name: gateway-secrets
    mountPath: /etc/ai-gateway/secrets.yaml
    subPath: secrets.yaml
```

No per-slot `secretKeyRef` list required.

### D6 — Optional env surface after migration

| Env | Purpose |
| --- | --- |
| `AI_GATEWAY_SECRETS_FILE` | Override secrets file path |
| `AI_GATEWAY__*` | Deployment config overrides (port, log level) — unchanged |

`dotenvy::dotenv()` may load **only** `AI_GATEWAY_SECRETS_FILE` for convenience,
or be removed; not required if paths are defaulted.

### D7 — Coordination

- `cost-class` / `budget-rank` stay in embedded `credentials.yaml` (beta.17).
- Secrets file must not define policy fields.

## Migration Plan

1. Implement secrets file loader + integrations block.
2. Wire `CredentialRegistry` and Helicone/AWS from secrets only.
3. Delete legacy env resolution code and tests that assert legacy paths.
4. Add `dev/secrets.local.example.yaml`; gitignore `dev/secrets.local.yaml`.
5. Update docs; mark `.env.template` deprecated or replace with secrets example.
6. Bump **`0.3.0-beta.18`**.

## Operator migration (one-time)

```bash
cp dev/secrets.local.example.yaml dev/secrets.local.yaml
# paste keys from old .env into credentials: / integrations:
rm .env   # or keep only AI_GATEWAY_SECRETS_FILE if non-default path
cargo run -- -c ai-gateway/config/local.yaml
```
