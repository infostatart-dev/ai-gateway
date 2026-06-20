## Context

The Infostart fork inherited Helicone's **sidecar** deployment model: when
`helicone.features != none` and `deployment_target` is sidecar,
`main.rs` awaits `ControlPlaneClient::connect()` (websocket to
`helicone.websocket-url`) **before** registering the HTTP gateway service.

Committed `local.yaml` sets `features: all` and `localhost:8585`, but:

- `SIDECAR.md` states Helicone Cloud sidecar is **no longer supported**
- `docs/configuration.md` documents `features: none` for self-hosted dev
- `infrastructure/compose.yaml` does not run anything on `:8585`

Operators running `cargo rl` see routers created in logs but `:8080` never opens.

**Strategic direction:** Helicone Cloud control plane is legacy. Infostart will
implement a **first-party control plane** (config push, org keys, router registry)
on a future milestone. **0.5.3** unblocks local and production sidecar HTTP
without waiting for either Helicone Jawn or the new plane.

## Goals / Non-Goals

**Goals:**

- HTTP gateway **always** reaches `server starting` / `:8080` bind in sidecar mode
  regardless of `helicone.features` or websocket reachability.
- Remove Helicone websocket connect from the **critical startup path** in 0.5.3.
- Align `local.yaml` with docs (`helicone.features: none`).
- Record control-plane deferral and Infostart roadmap in specs + docs.
- Ship as **release 0.5.3**.

**Non-Goals:**

- Implement Infostart control plane (separate future change).
- Remove `control_plane` module, websocket client, or Helicone header middleware.
- Cloud deployment `database-listener` behaviour.
- Helicone Cloud observability export (can remain stubbed / optional later).

## Decisions

### D1 — Do not register blocking control-plane client at startup (0.5.3)

**Choice:** Remove the `ControlPlaneClient::connect().await?` gate in `run_app`
for sidecar. Do **not** add `control-plane-client` to meltdown until a
first-party plane exists.

**Rationale:** Simplest fix; matches `SIDECAR.md` and operator expectations.
Background reconnect loop is useless without a server and still confuses logs.

**Alternatives rejected:**

| Alternative | Why not |
|-------------|---------|
| Background CP client with non-blocking initial connect | Still spams retries; no server in dev |
| Config flag `control-plane.enabled` default true | Extra knob; legacy Helicone path should stay off |
| Keep blocking; fix `local.yaml` only | Code still traps anyone with `features: all` |

### D2 — `helicone.features` no longer implies startup websocket

**Choice:** `is_auth_enabled()` (or a new helper) is **not** used to gate HTTP
startup. Features may still drive observability / prompts middleware when
implemented; websocket control plane is **deferred**.

**Rationale:** Today `features: all` effectively means "block on Helicone Jawn",
which is not a product requirement for this fork.

### D3 — Dev config defaults

**Choice:** `ai-gateway/config/local.yaml`:

```yaml
helicone:
  features: none
```

Remove `base-url` / `websocket-url` pointing at `localhost:8585` from the
committed dev file (defaults in `HeliconeConfig` remain for upstream parity).

### D4 — Future Infostart control plane (recorded, not built)

**Choice:** Document in `docs/deployment.md` (or new `docs/control-plane.md`
section) a placeholder contract:

- Sidecar gateway exposes HTTP immediately.
- Control plane (future) will push router/org state over websocket or gRPC.
- Helicone Cloud sidecar path is **deprecated**; migration = stay on
  `features: none` until Infostart plane ships.

No code stubs beyond a comment in `main.rs` pointing to this change.

### D5 — Version 0.5.3

Bump workspace `version`, `CHANGELOG.md` section `[0.5.3]` with fix + behaviour
note.

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| Operators relying on Helicone Jawn for dynamic router push | Document breaking behaviour in CHANGELOG; Helicone sidecar already marked legacy |
| Dead `control_plane` code rots | Future change owns plane; keep module, add integration test when plane exists |
| `features: all` silently does less than upstream Helicone | Document in configuration.md; fork divergence is intentional |

## Migration Plan

1. Merge 0.5.3 with code + config + docs.
2. Operators on Helicone sidecar: set `helicone.features: none` or run without
   CP until Infostart plane is available.
3. Rollback: revert `main.rs` gate removal (not recommended).

## Open Questions

- Should `helicone.features: auth` still enable any middleware without CP? (Defer;
  audit in follow-up.)
- Exact API of Infostart control plane — separate design when implementation
  starts.
