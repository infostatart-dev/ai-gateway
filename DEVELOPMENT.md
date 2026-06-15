## Local development

Maintained by [Infostart IT Lab](https://infostart.ru/lab/about/).

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) 1.91+
- [Docker](https://docs.docker.com/get-docker/) and [Docker Compose](https://docs.docker.com/compose/install/) (optional, for Redis/cache stack)

### Setup

1. **Clone the repository**

   ```bash
   git clone https://github.com/infostatart-dev/ai-gateway.git
   cd ai-gateway
   ```

2. **Environment**

   ```bash
   cp .env.template .env
   ```

   Fill in `AI_GATEWAY_CREDENTIAL_*` variables for the provider slots you want
   to enable. See [`.env.template`](.env.template) for naming conventions and
   [README.md](README.md) for configuration overview.

   Optional: set `HELICONE_CONTROL_PLANE_API_KEY` only if you enable Helicone
   Cloud observability (`helicone.features` in config).

3. **Start supporting services** (optional)

   ```bash
   cd infrastructure && docker compose up -d && cd ..
   ```

4. **Run the gateway**

   ```bash
   cargo run

   # Or with a dev config file:
   cargo rl
   ```

### Testing

```bash
# Send an HTTP request against the running gateway
cargo run -p test

# Unit and integration tests
cargo test --tests --all-features
```

### Build

```bash
cargo build          # debug
cargo build --release
```
