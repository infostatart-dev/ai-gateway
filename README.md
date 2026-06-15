# AI Gateway

High-performance LLM proxy and router written in Rust.

> **Fork notice.** Modified version based on
> [Helicone/ai-gateway](https://github.com/Helicone/ai-gateway) (Apache 2.0).
> Maintained and extended by
> [Infostart IT Lab](https://infostart.ru/lab/about/) — we validate DevOps
> practices and integrations on real production workloads.
> Follow our AI engineering talks at
> [Infostart events](https://infostart.ru/event/).

[![License](https://img.shields.io/badge/license-Apache--2.0-green?style=flat-square)](LICENSE)

## What this fork adds

- **Credential pool** — multiple upstream accounts per provider (`credentials.yaml` + `AI_GATEWAY_CREDENTIAL_*`)
- **Budget-aware routing** — failover, cooldowns, and rank scoring per credential
- **Upstream pacing** — concurrent, RPM, and min-interval gates (`provider-limits.yaml`)
- **Extended providers** — OpenRouter, Cloudflare Workers AI, ChatGPT Web session, and more
- **Structured JSON failover** — schema-aware routing with `X-RealMode-Model-And-Provider` response header
- **429 handling** — upstream-aware cooldowns from headers, JSON hints, and error text

See [CHANGELOG.md](CHANGELOG.md) for the full release history.

## Quick start

1. Copy the environment template and set provider credentials:

   ```bash
   cp .env.template .env
   # Fill AI_GATEWAY_CREDENTIAL_* variables — see .env.template for naming
   ```

2. Run locally:

   ```bash
   cargo run
   ```

3. Send a request with the OpenAI SDK:

   ```python
   from openai import OpenAI

   client = OpenAI(
       base_url="http://localhost:8080/ai",
       api_key="placeholder",  # gateway handles upstream keys
   )

   response = client.chat.completions.create(
       model="openai/gpt-4o-mini",
       messages=[{"role": "user", "content": "Hello!"}],
   )
   print(response.choices[0].message.content)
   ```

## Configuration

Embedded defaults live in `ai-gateway/config/embedded/`:

| File | Purpose |
|------|---------|
| [`credentials.yaml`](ai-gateway/config/embedded/credentials.yaml) | Upstream account slots and budget ranks |
| [`providers.yaml`](ai-gateway/config/embedded/providers.yaml) | Supported providers and models |
| [`provider-limits.yaml`](ai-gateway/config/embedded/provider-limits.yaml) | Rate limits, cooldowns, pacing |
| [`model-mapping.yaml`](ai-gateway/config/embedded/model-mapping.yaml) | Cross-provider model aliases |

**Documentation:**

- [Configuration overview](docs/configuration.md)
- [Credentials and env vars](docs/credentials.md)
- [Providers](docs/providers.md)
- [Routing and failover](docs/routing.md)
- [Deployment](docs/deployment.md)
- [ChatGPT Web session](docs/chatgpt-web.md)

Override with `-c path/to/config.yaml` or environment variables (see [`.env.template`](.env.template)).

## Architecture

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Your App      │───▶│   AI Gateway    │───▶│  LLM Providers  │
│                 │    │                 │    │                 │
│ OpenAI SDK      │    │ • Load balance  │    │ • OpenAI        │
│ (any language)  │    │ • Failover      │    │ • Anthropic     │
│                 │    │ • Rate limit    │    │ • Gemini        │
│                 │    │ • Cache         │    │ • OpenRouter    │
│                 │    │ • Pacing        │    │ • 20+ more      │
└─────────────────┘    └─────────────────┘    └─────────────────┘
                               │
                               ▼
                      ┌─────────────────┐
                      │ OpenTelemetry   │
                      │ metrics/traces  │
                      └─────────────────┘
```

## Migration from OpenAI SDK

### Python

```diff
 from openai import OpenAI

 client = OpenAI(
-    api_key=os.getenv("OPENAI_API_KEY"),
+    api_key="placeholder",
+    base_url="http://localhost:8080/router/your-router-name",
 )

 response = client.chat.completions.create(
-    model="gpt-4o-mini",
+    model="openai/gpt-4o-mini",
     messages=[{"role": "user", "content": "Hello!"}],
 )
```

### TypeScript

```diff
 import { OpenAI } from "openai";

 const client = new OpenAI({
-   apiKey: process.env.OPENAI_API_KEY,
+   apiKey: "placeholder",
+   baseURL: "http://localhost:8080/router/your-router-name",
 });

 const response = await client.chat.completions.create({
-  model: "gpt-4o",
+  model: "openai/gpt-4o",
   messages: [{ role: "user", content: "Hello!" }],
 });
```

## Docker

```bash
docker build -t ai-gateway .
docker run -p 8080:8080 --env-file .env ai-gateway
```

## Development

See [DEVELOPMENT.md](DEVELOPMENT.md) for local setup, testing, and contribution workflow.

Examples: [Python](examples/python/README.md) · [TypeScript](examples/typescript/README.md)

## License

Licensed under the [Apache License 2.0](LICENSE).

Based on [Helicone/ai-gateway](https://github.com/Helicone/ai-gateway), originally
released under Apache 2.0 by the Helicone developers.

Optional Helicone Cloud observability (`helicone.features` in config) remains
available but is **not required** for self-hosted operation.

Fork repository: [infostatart-dev/ai-gateway](https://github.com/infostatart-dev/ai-gateway)
