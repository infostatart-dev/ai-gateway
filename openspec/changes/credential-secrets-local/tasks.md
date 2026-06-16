## 1. Secrets file loader

- [x] 1.1 Add `secrets_file.rs` — parse `credentials` + `integrations` schema
- [x] 1.2 Discovery: `AI_GATEWAY_SECRETS_FILE` → `./dev/secrets.local.yaml` → `~/.config/ai-gateway/secrets.yaml`
- [x] 1.3 Resolve relative paths from secrets file directory

## 2. Wire loaders (breaking)

- [x] 2.1 `CredentialRegistry::build` reads secrets file only (remove env resolution)
- [x] 2.2 Apply `integrations.helicone.api-key` after config merge
- [x] 2.3 Apply `integrations.aws` for Bedrock; remove `AWS_*` env overrides in `read.rs`
- [x] 2.4 Remove `HELICONE_CONTROL_PLANE_API_KEY` from `helicone/mod.rs` default

## 3. Remove legacy code

- [x] 3.1 Delete or gut `credential_env.rs` legacy paths and associated tests
- [x] 3.2 Remove `CHATGPT_BROWSER_CLI` / `DEEPSEEK_BROWSER_CLI` env fallbacks in web config modules
- [x] 3.3 Remove or minimize `dotenvy::dotenv()` in `main.rs`

## 4. Dev artifacts

- [x] 4.1 Add `dev/secrets.local.example.yaml`
- [x] 4.2 Gitignore `dev/secrets.local.yaml`
- [x] 4.3 Replace `.env.template` with pointer to secrets example + `local.yaml`

## 5. Docs

- [x] 5.1 Rewrite `docs/credentials.md` — secrets file only, breaking change note
- [x] 5.2 Update `docs/configuration.md` — no `.env` happy path; Helicone key in secrets

## 6. Tests and release

- [x] 6.1 Tests: secrets discovery, integrations, legacy env ignored
- [x] 6.2 `mise exec -- openspec validate credential-secrets-local --strict`
- [x] 6.3 Bump `Cargo.toml` **`0.3.0-beta.18`**
