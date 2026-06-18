# Credentials

The gateway loads upstream API keys and browser session paths from a **single
secrets YAML file**, not from scattered environment variables.

Provider **policy** (tier, budget-rank, cost-class) stays in embedded
[`credentials.yaml`](../ai-gateway/config/embedded/credentials.yaml). Each
slot represents one upstream account or billing tier (for example
`openrouter-default`, `gemini-free`). The budget-aware router treats each slot
as a separate candidate for failover and cooldown tracking.

**Per-model exhaustion (Gemini free):** when only one model slug on a slot hits
RPM/RPD, the router retires `(credential, model)` — not the whole slot. Project
billing / spending-cap 429 still retires the slot and skips free-tier siblings.
See `provider-ladders.yaml` for same-slot model escalation order. Embedded
upstream slugs are verified against a frozen ListModels fixture — see
[providers.md](providers.md#gemini-catalog-verify-042-beta3).

> **Breaking change (0.3.0-beta.18):** `AI_GATEWAY_CREDENTIAL_*`,
> `{PROVIDER}_API_KEY`, `GEMINI_FREE_TIER_*`, `CHATGPT_BROWSER_CLI`,
> `DEEPSEEK_BROWSER_CLI`, `HELICONE_CONTROL_PLANE_API_KEY`, and `AWS_*` env
> overrides are **no longer read**. Migrate to the secrets file below.

## Secrets file

Copy the example and fill in real values:

```bash
cp dev/secrets.local.example.yaml dev/secrets.local.yaml
```

Discovery order (first existing file wins):

1. `AI_GATEWAY_SECRETS_FILE` — explicit path override
2. `./dev/secrets.local.yaml` — local development default
3. `~/.config/ai-gateway/secrets.yaml` — user-wide fallback

### Schema

```yaml
credentials:
  openrouter-default:
    api-key: sk-or-...
  gemini-free:
    api-key: ...
  cloudflare-default:
    api-key: "account_id:cfut_..."
  deepseek-web-default:
    session-file: dev/deepseek-session.json
  chatgpt-web-default:
    session-file: dev/session.json

integrations:
  helicone:
    api-key: sk-helicone-...
  aws:
    access-key: ...
    secret-key: ...
    region: eu-central-1
```

Per slot, use **one** of:

| Field | Use |
|-------|-----|
| `api-key` | Inline API key or Cloudflare `account_id:token` |
| `api-key-file` | Path to a file containing the key (relative to secrets file dir) |
| `session-file` | Path to browser session JSON (web providers) |

Policy fields (`tier`, `budget-rank`, `cost-class`, `provider`) are **not**
accepted in the secrets file.

If no secret is found for a slot, **the slot is skipped at startup** — no
error, the provider simply has fewer credentials available.

## Provider-specific formats

### Cloudflare Workers AI

Combined account and token in one value:

```yaml
credentials:
  cloudflare-default:
    api-key: "account_id:cfut_..."
```

### Gemini free siblings

Sixteen free-tier AI Studio slots share `tier: free` and equal `budget-rank` in
embedded policy. Set each key under its own slot id (`gemini-free` through
`gemini-free-16`):

```yaml
credentials:
  gemini-free:
    api-key: ...
  gemini-free-2:
    api-key: ...
  gemini-free-3:
    api-key: ...
  gemini-free-4:
    api-key: ...
  gemini-free-5:
    api-key: ...
  # … gemini-free-6 … gemini-free-15 …
  gemini-free-16:
    api-key: ...
  gemini-default:
    api-key: ...
```

### ChatGPT Web

Session file path in secrets — cost-class **`paid-browser`**. Autodefault tries
ChatGPT Web **last**. See [chatgpt-web.md](chatgpt-web.md).

```bash
cargo run --features chatgpt-login -p ai-gateway -- chatgpt login
```

CLI writes to `dev/session.json` by default; point `session-file` in secrets
to that path (or another path you prefer).

### DeepSeek Web

Session file with `userToken` from chat.deepseek.com — cost-class **`free`**.
Up to two browser sessions (`deepseek-web-default`, `deepseek-web-2`) round-robin
with isolated pacing gates.
See [deepseek-web.md](deepseek-web.md).

```bash
cargo run --features deepseek-login -p ai-gateway -- deepseek login
cargo run --features deepseek-login -p ai-gateway -- deepseek import \
  --token 'your-userToken'
```

Default session path: `dev/deepseek-session.json`.

### Tier 1 free API providers

Add API keys under slot ids from embedded `credentials.yaml`:

```yaml
credentials:
  longcat-default:
    api-key: ...
  bazaarlink-default:
    api-key: ...
  sambanova-default:
    api-key: ...
  ollama-cloud-default:
    api-key: ...
  cohere-default:
    api-key: ...
```

All use `tier: free` / `cost-class: free`. `groq-default` is also free-tier
(developer plan, no credit card). See [providers.md](providers.md) for base URLs
and model prefixes.

### AWS Bedrock

Optional `integrations.aws` block sets region URL and registers Bedrock
credentials when complete. No `AWS_ACCESS_KEY` / `AWS_REGION` env overrides.

## Helicone API key

Helicone **URLs and features** stay in `config/local.yaml`. The API key lives
only under `integrations.helicone.api-key` in the secrets file.

## Budget rank

Each slot has a `budget-rank` in embedded YAML. **Lower values are preferred
first** within the same provider when the budget-aware router selects
candidates.

See embedded config: all sixteen `gemini-free*` slots (rank 0) are tried before
`gemini-default` (rank 10) when both are eligible.

## Startup behaviour

At startup, `CredentialRegistry`:

1. Parses embedded `credentials.yaml` (policy only)
2. Loads the secrets file and resolves keys / session paths
3. Skips slots without secrets or whose provider is absent from `providers.yaml`
4. Registers Bedrock from `integrations.aws` when configured

## Production (Kubernetes)

Mount one Secret as a file and set:

```yaml
env:
  - name: AI_GATEWAY_SECRETS_FILE
    value: /etc/ai-gateway/secrets.yaml
```

## Adding a new slot

1. Add policy entry to embedded `credentials.yaml` (or fork embedded files).
2. Add matching `credentials.<slot-id>` entry in the secrets file.
3. Ensure the provider exists in `providers.yaml`.
4. Restart the gateway.

## Related

- [configuration.md](configuration.md) — config file layout
- [providers.md](providers.md) — provider catalogue
- [routing.md](routing.md) — how credentials participate in failover
