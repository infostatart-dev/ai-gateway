# Configuration

## How config is loaded

The gateway merges settings from several layers (later layers override earlier
ones):

1. **Built-in defaults** (Rust `Config::default()`)
2. **Config file** — path from `-c` / `--config`, or
   `/etc/ai-gateway/config.yaml` if it exists
3. **Environment variables** with prefix `AI_GATEWAY__`, kebab-case keys, nested
   separator `__`

   Example: `AI_GATEWAY__SERVER__PORT=9090`

On load, the gateway also:

- Loads the [`secrets file`](credentials.md) and builds
  [`CredentialRegistry`](credentials.md) from embedded `credentials.yaml` +
  secrets
- In **Sidecar** deployment mode, may inject an **autodefault** router when
  credentials are available (see [routing.md](routing.md))

Local development uses `config/local.yaml` for non-secret settings plus
`dev/secrets.local.yaml` for keys. No `.env` file is required.

**Telemetry:** committed `local.yaml` uses `exporter: stdout` so `cargo rl`
works without `docker compose` (OTEL collector on `:4317`). Use
`exporter: both` only when `infrastructure/compose.yaml` otelcol is running.

## Embedded reference files

Shipped under `ai-gateway/config/embedded/` and compiled into the binary:

| File | Role |
|------|------|
| [`credentials.yaml`](../ai-gateway/config/embedded/credentials.yaml) | Upstream account slots |
| [`providers.yaml`](../ai-gateway/config/embedded/providers.yaml) | Provider base URLs, model lists, capabilities |
| [`provider-limits.yaml`](../ai-gateway/config/embedded/provider-limits.yaml) | Cooldown defaults, RPM/concurrent pacing, tier notes |
| [`model-mapping.yaml`](../ai-gateway/config/embedded/model-mapping.yaml) | Cross-provider model aliases for failover |

These are the source of truth unless you maintain a custom build with edited
embedded files. Runtime overrides apply to **router/config YAML**, not to
embedded provider catalogues (without a rebuild).

## Example config files

Under `ai-gateway/config/`:

| File | Use |
|------|-----|
| [`local.yaml`](../ai-gateway/config/local.yaml) | Local dev with custom routers |
| [`decision-example.yaml`](../ai-gateway/config/decision-example.yaml) | Decision engine / tier cascade |
| [`helicone-cloud.yaml`](../ai-gateway/config/helicone-cloud.yaml) | Docker default path (`/etc/ai-gateway/`) |

Run with a custom file:

```bash
cargo run -- -c ai-gateway/config/local.yaml
```

## Routers

Named routers are defined under `routers:` in YAML. Each router has load-balance
strategy, optional rate limits, cache, and decision settings.

Request paths:

| Path | Description |
|------|-------------|
| `/ai/chat/completions` | Unified API (default router selection) |
| `/router/{name}/chat/completions` | Named router |
| `/router/autodefault/chat/completions` | Auto-built budget-aware router (Sidecar) |

Router IDs must match `ROUTER_ID_REGEX` (lowercase alphanumeric + hyphens).

## Autodefault router

When `deployment_target` is **Sidecar** and credentials exist, the gateway
builds an `autodefault` router automatically. Provider order (first available
wins priority):

1. `chatgpt-web` (if session file present)
2. `opencode`, `openrouter`, `mistral`, `groq`, `cerebras`, `cloudflare`,
   `gemini`, `anthropic`

Strategy: `budget-aware-capability-after` with decision engine enabled and
`tier-cascade: free-up`.

Startup banner prints tier, cascade mode, and fallback chain when autodefault is
active.

## Global middleware

`global:` and `unified-api:` sections configure middleware (cache, rate limit,
etc.) applied across routes. See upstream Helicone config shape for field names;
this fork preserves YAML compatibility.

## Helicone Cloud block (optional)

```yaml
helicone:
  features: none   # or all, observability, prompts, auth
```

Helicone API key: `integrations.helicone.api-key` in
[`dev/secrets.local.yaml`](credentials.md). **Not required** for self-hosted
operation. Sidecar mode with Helicone Cloud is [legacy](../SIDECAR.md).

**Startup:** HTTP bind does **not** wait for a control-plane websocket (see
[control-plane.md](control-plane.md)). `cargo rl` works without any service on
port `8585`.

## Local secrets setup

```bash
cp dev/secrets.local.example.yaml dev/secrets.local.yaml
# edit keys, then:
cargo run -- -c ai-gateway/config/local.yaml
```

See [credentials.md](credentials.md) for the full secrets schema and breaking
change notes for legacy env vars.

## Related

- [providers.md](providers.md)
- [routing.md](routing.md)
- [control-plane.md](control-plane.md)
- [deployment.md](deployment.md)
