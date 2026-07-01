## 1. Config and Data Model

- [x] 1.1 Add `client-access` config structs with `enabled`, `file`, `reload-interval`, `max-body-bytes`, and `quota-store`
- [x] 1.2 Add YAML registry structs for `version`, `subjects`, `plans`, `keys`, limits, statuses, timestamps, and scopes
- [x] 1.3 Implement strict YAML validation: deny unknown fields, require hash-only keys, validate references, validate nonzero limits
- [x] 1.4 Add quota store config variants for `memory` and `redis` using existing Redis config conventions
- [x] 1.5 Add `ClientAccessContext` extension type and keep `AuthContext` derivation isolated to compatibility code

## 2. Registry Loading and Reload

- [x] 2.1 Implement SHA-256 inbound key hash parsing and lookup with explicit `sha256:<hex>` format
- [x] 2.2 Implement immutable `ClientAccessSnapshot` with key lookup, subject resolution, plan resolution, and effective limits
- [x] 2.3 Wire initial registry loading into app startup when `client-access.enabled` is true
- [x] 2.4 Fail startup closed when enabled registry file is missing, unreadable, or invalid
- [x] 2.5 Add snapshot holder to `AppState` with non-async hot-path access
- [x] 2.6 Implement polling live reload task with metadata/change detection and configurable interval
- [x] 2.7 Implement last-good snapshot behavior for invalid reloads and deleted files
- [x] 2.8 Add reload success/failure logs and metrics without exposing raw keys

## 3. Inbound Access Middleware

- [x] 3.1 Add client access middleware after route classification and before router/unified/direct dispatch
- [x] 3.2 Extract Bearer token, hash it, and reject missing or unknown keys with OpenAI-shaped 401 errors
- [x] 3.3 Enforce key status and `expires-at` before quota checks
- [x] 3.4 Attach `ClientAccessContext` to authenticated protected requests
- [x] 3.5 Derive legacy `AuthContext` only for downstream compatibility after successful client access auth
- [x] 3.6 Enforce `unified-api`, `router:<id>`, `direct:<provider>`, and `*` scopes before quota checks
- [x] 3.7 Keep health and explicitly public endpoints outside client access enforcement
- [x] 3.8 Preserve legacy Helicone/control-plane auth path only when `client-access.enabled` is false

## 4. Quota Domain and Memory Store

- [x] 4.1 Define quota dimensions for requests/tokens across minute, day, and week windows
- [x] 4.2 Implement window clock helpers: rolling 60-second minute, UTC day reset hour, ISO week
- [x] 4.3 Define quota store trait for request admission, token reservation, commit, refund, and rejection metadata
- [x] 4.4 Implement in-memory quota store matching the specified window semantics
- [x] 4.5 Add per-key isolation in memory store for request counters, token counters, and reservations
- [x] 4.6 Add warning when memory store is used with production/cloud-style config

## 5. Redis Quota Store

- [x] 5.1 Implement Redis quota store construction from `client-access.quota-store`
- [x] 5.2 Define Redis key schema for per-key request windows, token windows, and token reservations
- [x] 5.3 Implement atomic Redis admission/reservation operation that checks all applicable dimensions together
- [x] 5.4 Ensure rejected Redis admissions leave no partial request/token counter increments
- [x] 5.5 Implement Redis commit/refund for token reservations, including over-reservation refund and usage debt
- [x] 5.6 Add TTL handling for minute/day/week keys and stale reservation cleanup
- [x] 5.7 Return fail-closed 503 for quota-protected traffic when Redis backend is unavailable

## 6. Token Admission and Settlement

- [x] 6.1 Add protected request body buffering with `client-access.max-body-bytes`
- [x] 6.2 Reuse existing token estimation to calculate input tokens for chat requests
- [x] 6.3 Resolve reserved output tokens from request max token fields or plan `max-output-tokens`
- [x] 6.4 Reserve request and token quota before upstream dispatch
- [x] 6.5 Refund reservations when dispatch fails before an upstream attempt
- [x] 6.6 Commit reported usage on non-streaming responses when provider usage is available
- [x] 6.7 Commit estimated usage when successful response usage is unavailable
- [x] 6.8 Wrap streaming response bodies so reservations settle on completion or error
- [x] 6.9 Record usage over reservation as quota debt for future admission

## 7. Error Contract, Headers, and Metrics

- [x] 7.1 Add OpenAI-shaped 401, 403, 429, and 503 client access error mapping
- [x] 7.2 Add `retry-after` and rate-limit limit/remaining headers for quota rejections
- [x] 7.3 Add successful response rate-limit headers for the most constrained known dimension
- [x] 7.4 Add metrics for auth attempts, auth rejections, scope denials, quota admissions, quota rejections, and Redis quota errors
- [x] 7.5 Ensure logs/metrics include key id, plan id, and dimension but never raw inbound API keys

## 8. Documentation and Examples

- [x] 8.1 Add `dev/client-access.local.example.yaml` with hash-only keys and starter plan examples
- [x] 8.2 Document client access config in `docs/configuration.md`
- [x] 8.3 Add a client access guide covering key generation, hash storage, scopes, limits, reload, and Redis backend
- [x] 8.4 Update control-plane docs to state client access is first-party inbound auth while Helicone auth remains legacy compatibility

## 9. Tests and Verification

- [x] 9.1 Add unit tests for YAML parsing, validation failures, hash parsing, and snapshot lookup
- [x] 9.2 Add live reload tests for valid revocation, invalid YAML last-good behavior, and deleted file behavior
- [x] 9.3 Add middleware tests for missing key, invalid key, suspended key, expired key, context insertion, and legacy auth fallback
- [x] 9.4 Add scope tests for unified API, named router, direct provider, wildcard, and denied routes
- [x] 9.5 Add memory quota tests for request minute/day/week windows and per-key isolation
- [x] 9.6 Add token quota tests for reservation, refund, reported usage commit, estimated usage commit, overage debt, and body-size rejection
- [x] 9.7 Add Redis quota tests for cross-instance shared limits, atomic rejection without partial increments, TTLs, and Redis unavailable fail-closed behavior
- [x] 9.8 Add streaming settlement tests for success and early error paths
- [x] 9.9 Run focused `cargo test` suites for config, client access middleware, quota store, and routing harness coverage
