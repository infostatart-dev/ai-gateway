# Credentials

The gateway loads upstream API keys from environment variables and maps them to
**credential slots** defined in
[`credentials.yaml`](../ai-gateway/config/embedded/credentials.yaml).

Each slot represents one upstream account or billing tier (for example
`openai-default`, `gemini-free`). The budget-aware router treats each slot as a
separate candidate for failover and cooldown tracking.

## Environment variable naming

Primary convention:

```
AI_GATEWAY_CREDENTIAL_<ID>
```

The credential `id` from YAML is uppercased; hyphens become underscores.

| Slot ID in YAML | Environment variable |
|-----------------|----------------------|
| `openai-default` | `AI_GATEWAY_CREDENTIAL_OPENAI_DEFAULT` |
| `gemini-free` | `AI_GATEWAY_CREDENTIAL_GEMINI_FREE` |
| `gemini-free-2` | `AI_GATEWAY_CREDENTIAL_GEMINI_FREE_2` |
| `gemini-free-3` | `AI_GATEWAY_CREDENTIAL_GEMINI_FREE_3` |
| `gemini-free-4` | `AI_GATEWAY_CREDENTIAL_GEMINI_FREE_4` |
| `cloudflare-default` | `AI_GATEWAY_CREDENTIAL_CLOUDFLARE_DEFAULT` |

See [`.env.template`](../.env.template) for a full starter list.

## Resolution order

For each slot, the gateway tries env vars in this order:

1. `AI_GATEWAY_CREDENTIAL_<ID>` (universal)
2. Optional `key-env` / `alt-key-envs` from YAML (if defined on the slot)
3. Legacy `{PROVIDER}_API_KEY` — only for slots whose id ends with `-default`
   (for example `OPENAI_API_KEY` for `openai-default`)
4. Provider-specific legacy names (see below)

If no secret is found, **the slot is skipped at startup** — no error, the
provider simply has fewer credentials available.

## Provider-specific formats

### Cloudflare Workers AI

Combined account and token in one value:

```bash
AI_GATEWAY_CREDENTIAL_CLOUDFLARE_DEFAULT="account_id:cfut_..."
```

Legacy fallbacks: `CLOUDFLARE_API_KEY_WITH_ACCOUNT_ID`, or separate
`CLOUDFLARE_ACCOUNT_ID` + `CLOUDFLARE_API_KEY`.

### Gemini

Four free-tier AI Studio slots share `tier: free` and equal `budget-rank`; set
each key via its own `AI_GATEWAY_CREDENTIAL_GEMINI_FREE*` env var. Legacy
aliases apply only to the first slot (`gemini-free`).

| Slot | Environment variable | Legacy fallbacks |
|------|----------------------|------------------|
| `gemini-free` | `AI_GATEWAY_CREDENTIAL_GEMINI_FREE` | `GEMINI_FREE_TIER_API_KEY`, `GEMINI_FREE_TIER_APIKEY` |
| `gemini-free-2` | `AI_GATEWAY_CREDENTIAL_GEMINI_FREE_2` | — |
| `gemini-free-3` | `AI_GATEWAY_CREDENTIAL_GEMINI_FREE_3` | — |
| `gemini-free-4` | `AI_GATEWAY_CREDENTIAL_GEMINI_FREE_4` | — |
| `gemini-default` | `AI_GATEWAY_CREDENTIAL_GEMINI_DEFAULT` | `GEMINI_API_KEY` |

### ChatGPT Web

Session file path in `AI_GATEWAY_CREDENTIAL_CHATGPT_WEB_DEFAULT` (value = path
to session JSON). Fallback env: `CHATGPT_BROWSER_CLI`. Cost-class:
**`paid-browser`** — autodefault tries ChatGPT Web **last**, after free API
keys and paid API fallbacks. See [chatgpt-web.md](chatgpt-web.md).

### DeepSeek Web

Session file with `userToken` from chat.deepseek.com localStorage — see
[deepseek-web.md](deepseek-web.md). Cost-class: **`free`**, ordered **after**
Gemini free slots and **before** paid `gemini-default`.

```bash
cargo run --features deepseek-login -p ai-gateway -- deepseek login
cargo run --features deepseek-login -p ai-gateway -- deepseek import \
  --token 'your-userToken'
cargo run --features deepseek-login -p ai-gateway -- deepseek probe
```

| Slot | Env var (value = path to session JSON) |
|------|----------------------------------------|
| `chatgpt-web-default` | `AI_GATEWAY_CREDENTIAL_CHATGPT_WEB_DEFAULT` |
| `deepseek-web-default` | `AI_GATEWAY_CREDENTIAL_DEEPSEEK_WEB_DEFAULT` |

CLI writes to `DEEPSEEK_BROWSER_CLI` (default account path).

### Perplexity Web (dormant in catalog)

Session file with logged-in `__Secure-next-auth.session-token` (+ CF cookies).
The crate and CLI remain; provider is disabled in embedded catalog until
dispatcher integration is complete.

```bash
cargo run --features perplexity-login -p ai-gateway -- perplexity login
cargo run --features perplexity-login -p ai-gateway -- perplexity import \
  --cookie 'Cookie: __Secure-next-auth.session-token=...; cf_clearance=...'
```

| Slot | Env var (value = path to session JSON) |
|------|----------------------------------------|
| `perplexity-web-default` | (removed from embedded catalog) |

CLI writes to `PERPLEXITY_BROWSER_CLI` when used manually.

## Budget rank

Each slot has a `budget-rank` in YAML. **Lower values are preferred first**
within the same provider when the budget-aware router selects candidates.
Multiple slots with the same provider and model are **round-robin balanced**
across requests; on failure the router tries sibling accounts before moving to
the next provider.

Example from embedded config: all four `gemini-free*` slots (rank 0) are tried
before `gemini-default` (rank 10) when both are eligible. Configured free
siblings round-robin across requests. On **transient RPM** `429`, the router
tries the next free sibling. On **daily quota exhaustion** (`429
RESOURCE_EXHAUSTED`) or **503 overload**, remaining free siblings are skipped
for that request and the router jumps to `gemini-default` or the next provider
instead of burning every free key on the same body.

Failover and cooldown metrics include `credential` and `quota_metric`
(`rpm|tpm|rpd|overload`) attributes; each request emits a structured
`budget-aware route summary` log (hops, duration, terminal outcome).

## Startup behaviour

At startup, `CredentialRegistry`:

1. Parses embedded `credentials.yaml`
2. Resolves secrets from the environment
3. Skips slots without secrets or whose provider is absent from `providers.yaml`
4. Adds session-based credentials (ChatGPT Web) when a valid session file exists

If **no credentials** resolve for any provider you need, requests to that
provider will fail at routing time.

## Adding a new slot

1. Add an entry to `credentials.yaml` (or your override config if you fork
   embedded files).
2. Set the matching `AI_GATEWAY_CREDENTIAL_*` env var.
3. Ensure the provider exists in `providers.yaml`.
4. Restart the gateway.

## Related

- [configuration.md](configuration.md) — config file layout
- [providers.md](providers.md) — provider catalogue
- [routing.md](routing.md) — how credentials participate in failover
