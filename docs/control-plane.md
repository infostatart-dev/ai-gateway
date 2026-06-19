# Control plane

## Current state (0.5.3+)

The Infostart fork **does not** connect to Helicone Cloud Jawn on startup.
HTTP on `server.port` (default `:8080`) binds immediately in sidecar mode,
regardless of `helicone.features` or websocket reachability.

Helicone Cloud **sidecar** mode (dynamic router push over websocket) is
[legacy](../SIDECAR.md) and not supported in this fork.

## What still works without a control plane

- Named routers from config YAML (`/router/{name}/…`)
- Sidecar **autodefault** router (built from credential slots)
- Budget-aware routing, pacing, failover, provider-stats
- Caller context headers (`Helicone-Session-Id`, etc.) — compatibility only

## Helicone block in config

```yaml
helicone:
  features: none   # recommended for local dev and self-hosted sidecar
```

Optional Helicone API key: `integrations.helicone.api-key` in the
[secrets file](credentials.md). Not required for routing.

Setting `features` to `all`, `observability`, or `auth` does **not** block HTTP
startup in 0.5.3+. Observability middleware may evolve separately; websocket
control plane is off until the Infostart plane ships.

## Roadmap: Infostart control plane

A **first-party control plane** is planned for a future release. It will:

- Push router and organization configuration to sidecar gateways
- Replace the legacy Helicone Cloud websocket path
- Run as a separate service (websocket or gRPC — TBD)

Until then, configure routers in YAML or use autodefault from embedded
credentials + secrets file.

## Migration from Helicone sidecar

1. Set `helicone.features: none` in config.
2. Remove `localhost:8585` URLs unless you run legacy Helicone Jawn yourself.
3. Manage routers via config files or wait for the Infostart control plane.

## Related

- [Configuration](configuration.md)
- [Deployment](deployment.md)
- [SIDECAR.md](../SIDECAR.md) — legacy Helicone sidecar notice
