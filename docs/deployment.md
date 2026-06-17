# Deployment

## Local / bare metal

```bash
cp .env.template .env
# set AI_GATEWAY_CREDENTIAL_* variables

cargo build --release -p ai-gateway
./target/release/ai-gateway
```

Optional config file:

```bash
./target/release/ai-gateway -c /path/to/config.yaml
```

Default config path when `-c` is omitted: `/etc/ai-gateway/config.yaml` if the
file exists. Local `cargo run` uses built-in defaults plus `.env`.

## Docker

Build from repository root:

```bash
docker build -t ai-gateway .
docker run -p 8080:8080 --env-file .env ai-gateway
```

The runtime image:

- Exposes port **8080**
- Installs binary at `/usr/local/bin/ai-gateway`
- Copies [`helicone-cloud.yaml`](../ai-gateway/config/helicone-cloud.yaml) to
  `/etc/ai-gateway/helicone-cloud.yaml` as default config

Override config by mounting a file:

```bash
docker run -p 8080:8080 --env-file .env \
  -v $(pwd)/ai-gateway/config/local.yaml:/etc/ai-gateway/config.yaml \
  ai-gateway
```

## Private registry (CI)

GitHub Actions workflow [`.github/workflows/docker.yml`](../.github/workflows/docker.yml)
builds and pushes on `main` and version tags when secrets are configured:

- `DOCKER_REGISTRY` — registry host
- `DOCKER_REGISTRY_USERNAME` / `DOCKER_REGISTRY_PASSWORD` — login

Image tags follow `docker/metadata-action` defaults (branch, semver, sha).

## GitHub releases

Two release channels:

| Channel | Workflow | When |
| --- | --- | --- |
| **Versioned** (`v0.3.0-beta.19`, …) | [`release.yml`](../.github/workflows/release.yml) | Tag `v*` exists |
| **Rolling `latest`** | [`release-latest.yml`](../.github/workflows/release-latest.yml) | After each green Rust CI on `main` |

### Cut a versioned release

1. Bump `[workspace.package].version` in root [`Cargo.toml`](../Cargo.toml) and update [`CHANGELOG.md`](../CHANGELOG.md).
2. Push **only** `main` (do not push `v*` tags manually). Workflow [`version-tag.yml`](../.github/workflows/version-tag.yml) creates `v{version}` at HEAD and dispatches `release.yml` plus `docker.yml` for that tag.
3. Binaries appear on the GitHub Releases page as `ai-gateway-v{version}-{linux,darwin,windows}`.

After Rust CI on `main` finishes, [`release-latest.yml`](../.github/workflows/release-latest.yml) and [`docker.yml`](../.github/workflows/docker.yml) publish the rolling **`latest`** channel for the same commit.

To backfill a tag for the current version without changing `Cargo.toml`, run **Version tag** manually (`workflow_dispatch`) on GitHub Actions.

To rerun semver binaries or Docker for an existing tag, use **workflow_dispatch** on **Release binaries** or **Docker** with ref `v{version}`.

Prerelease tags (names containing `-`, e.g. `-beta.`) are published as GitHub prereleases and are not marked “Latest”.

## Environment variables

### Required for providers you use

`AI_GATEWAY_CREDENTIAL_*` — see [credentials.md](credentials.md) and
[`.env.template`](../.env.template).

### Optional

| Variable | Purpose |
|----------|---------|
| `HELICONE_CONTROL_PLANE_API_KEY` | Helicone Cloud observability/auth (optional) |
| `CHATGPT_BROWSER_CLI` | Path to ChatGPT Web session JSON |
| `OTEL_METRIC_EXPORT_INTERVAL` | OpenTelemetry metrics export interval (ms) |
| `AWS_ACCESS_KEY` / `AWS_SECRET_KEY` | AWS Bedrock |
| `AI_GATEWAY__*` | Override any config key (see [configuration.md](configuration.md)) |

## OpenTelemetry

Configure via `telemetry:` section in YAML. Exporters: stdout, OTLP, or both.
Set `OTEL_METRIC_EXPORT_INTERVAL` in `.env` for export cadence.

## Health check

```bash
curl http://localhost:8080/health
```

## Supporting services

Redis / Postgres stacks for cache and rate-limit persistence are optional for
basic operation. See `infrastructure/docker compose` for local dev dependencies.

## Related

- [configuration.md](configuration.md)
- [DEVELOPMENT.md](../DEVELOPMENT.md)
