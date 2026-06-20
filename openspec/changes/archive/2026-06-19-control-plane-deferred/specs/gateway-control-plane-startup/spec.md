## ADDED Requirements

### Requirement: HTTP gateway starts without control-plane websocket

In sidecar deployment mode, the gateway SHALL bind the HTTP server and accept
router traffic without awaiting a successful Helicone or external control-plane
websocket connection.

#### Scenario: Local dev with helicone features all
- **WHEN** `deployment_target` is sidecar
- **AND** `helicone.features` is `all`
- **AND** `helicone.websocket-url` is unreachable
- **THEN** the gateway logs `server starting` and listens on `server.port`
- **AND** `GET /health` returns success within normal startup time

#### Scenario: cargo rl default config
- **WHEN** the operator runs `cargo rl` with committed `local.yaml`
- **THEN** the gateway reaches HTTP ready state without external services on port 8585

### Requirement: Control-plane client not on critical startup path

The gateway SHALL NOT register `control-plane-client` as a meltdown service that
must complete websocket connect before the `gateway` HTTP service is registered.

#### Scenario: Meltdown task list at startup
- **WHEN** sidecar mode starts with any `helicone.features` value
- **THEN** meltdown `starting services` task list includes `gateway`
- **AND** does not include `control-plane-client` in release 0.5.3

### Requirement: Infostart control plane deferred

The gateway SHALL document that Helicone Cloud sidecar control plane is legacy
and a first-party Infostart control plane will replace it in a future release.
Release 0.5.3 SHALL NOT require any control-plane server for routing or
autodefault.

#### Scenario: Operator reads deployment docs
- **WHEN** an operator follows `docs/configuration.md` for local development
- **THEN** documentation states control plane is optional in 0.5.3
- **AND** points to future Infostart-owned control plane

### Requirement: Committed local dev config

Committed `ai-gateway/config/local.yaml` SHALL set `helicone.features: none` and
SHALL NOT reference `localhost:8585` for Helicone base or websocket URLs.

#### Scenario: Fresh clone dev config
- **WHEN** a contributor opens `ai-gateway/config/local.yaml` on `main` after 0.5.3
- **THEN** `helicone.features` is `none`
