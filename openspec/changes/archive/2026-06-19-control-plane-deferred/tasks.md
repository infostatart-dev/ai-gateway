## 1. Startup gate removal

- [x] 1.1 Remove sidecar `ControlPlaneClient::connect().await?` block from `main.rs` `run_app` (no `control-plane-client` meltdown registration in 0.5.3)
- [x] 1.2 Add brief comment in `main.rs` referencing deferred Infostart control plane (`design.md` / `docs/control-plane.md`)
- [x] 1.3 Verify meltdown task list at startup: `gateway` present, `control-plane-client` absent

## 2. Dev config alignment

- [x] 2.1 Set `helicone.features: none` in `ai-gateway/config/local.yaml`
- [x] 2.2 Remove `localhost:8585` `base-url` / `websocket-url` from committed `local.yaml`
- [x] 2.3 Confirm `emulated.yaml` and `sidecar.yaml` remain consistent with new policy

## 3. Tests

- [x] 3.1 Add integration or unit test: sidecar config with `helicone.features: all` and unreachable websocket does not block `App` / HTTP service registration (mock or timeout-bound)
- [x] 3.2 Run `cargo test` for affected startup paths; update any test expecting `control-plane-client` in meltdown tasks

## 4. Documentation

- [x] 4.1 Update `docs/configuration.md` — Helicone features optional; no startup websocket gate
- [x] 4.2 Update `DEVELOPMENT.md` — `cargo rl` works without `:8585`; docker compose does not include Helicone
- [x] 4.3 Add `docs/control-plane.md` (or section in `deployment.md`) — Helicone legacy, Infostart control plane roadmap
- [x] 4.4 Cross-link `SIDECAR.md` from configuration docs

## 5. Release 0.5.3

- [x] 5.1 Bump workspace `version` to `0.5.3` in root `Cargo.toml` / lockfile members
- [x] 5.2 Add `[0.5.3]` section to `CHANGELOG.md` (fix: HTTP startup no longer blocked by Helicone CP; local.yaml defaults)
- [x] 5.3 `openspec validate control-plane-deferred --strict`
