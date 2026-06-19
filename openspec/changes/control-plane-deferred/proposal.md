## Why

`cargo rl` loads `local.yaml` with `helicone.features: all`, which blocks HTTP startup
until a Helicone control-plane websocket on `:8585` connects. That service is not part of
this fork's dev stack (`SIDECAR.md` marks Helicone sidecar as legacy), so local gateway
never binds `:8080` — routing and autodefault appear broken when the real failure is
startup gating. We need immediate relief in **0.5.3** and a recorded path to an
**Infostart-owned control plane** later.

## What Changes

- **Stop blocking gateway HTTP startup** on Helicone control-plane websocket connect in
  sidecar mode; control-plane client becomes optional / background (or disabled) until our
  own implementation ships.
- Align **`ai-gateway/config/local.yaml`** with documented dev defaults
  (`helicone.features: none`).
- Document **deferred control-plane roadmap** (Helicone legacy → future Infostart control
  plane) in design and operator docs.
- Update **CHANGELOG** and workspace version to **0.5.3**.
- No removal of Helicone observability hooks or header compatibility (`Helicone-Session-Id`,
  etc.) — only the **startup gate** and committed dev config drift.

## Capabilities

### New Capabilities

- `gateway-control-plane-startup`: Startup and lifecycle requirements for control-plane
  integration — HTTP must start without external Helicone Jawn; future Infostart control
  plane is the target, not Helicone Cloud sidecar.

### Modified Capabilities

- `credential-secrets-local`: Clarify that `helicone.features` in config does **not** block
  HTTP server bind; Helicone API key in secrets remains optional for future observability.

## Impact

- `ai-gateway/src/main.rs` — control-plane registration / connect ordering
- `ai-gateway/src/control_plane/websocket.rs` — connect retry no longer on critical startup path
- `ai-gateway/config/local.yaml`, `docs/configuration.md`, `DEVELOPMENT.md`, `SIDECAR.md`
- Tests touching startup / meltdown service list
- Release: **v0.5.3**
