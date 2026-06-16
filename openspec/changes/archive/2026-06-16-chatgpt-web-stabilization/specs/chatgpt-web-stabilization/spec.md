## ADDED Requirements

### Requirement: Session warmup cache
The `chatgpt-web` executor SHALL cache successful session warmup for a bounded TTL and skip redundant warmup HTTP calls when the cache entry for the current session identity is still valid.

#### Scenario: Second request within TTL skips warmup GETs
- **WHEN** two chat completions run within 60 seconds
- **AND** they use the same session cookie and access token
- **THEN** the second completion does not perform the three warmup GET requests (`/me`, `/conversations`, `/models`)
- **AND** sentinel and conversation requests still run

#### Scenario: Warmup runs again after TTL expires
- **WHEN** a chat completion runs more than 60 seconds after the previous warmup for the same session identity
- **THEN** all three warmup GET requests run again before sentinel

#### Scenario: Cache is bounded
- **WHEN** more than 200 distinct session identities warm up in one process
- **THEN** the oldest cache entries are evicted before new ones are stored
- **AND** the process does not grow warmup cache without bound

### Requirement: Invalidate session caches on auth or abuse
The `chatgpt-web` executor SHALL clear per-session warmup and access-token cache entries when upstream auth fails or the session is treated as blocked.

#### Scenario: Auth failure clears warmup cache
- **WHEN** a chat completion receives HTTP 401 or 403 from session exchange, sentinel, or conversation
- **THEN** the access-token cache entry for that session cookie is invalidated
- **AND** the warmup cache entry for that cookie and access token is invalidated
- **AND** a subsequent completion within the former warmup TTL performs warmup GETs again

### Requirement: Session token rotation preserves helper cookies
The gateway SHALL merge rotated NextAuth session-token chunks without dropping Cloudflare helper cookies from the stored session blob.

#### Scenario: Unchunked to chunked rotation
- **WHEN** `merge_refreshed_cookie` receives Set-Cookie with chunked session tokens (`.0`, `.1`, …)
- **AND** the existing blob had a single unchunked `__Secure-next-auth.session-token`
- **THEN** the merged result contains only the refreshed token family members
- **AND** does not retain the stale unchunked token name alongside chunked pieces

#### Scenario: Cloudflare cookies survive rotation
- **WHEN** the existing cookie blob includes `cf_clearance` (or other allowed CF helpers)
- **AND** session tokens are rotated via Set-Cookie
- **THEN** the merged cookie blob still includes the CF helper cookies

### Requirement: Browser-like pacing for chatgpt-web
The gateway SHALL apply conservative upstream pacing for `chatgpt-web` so autodefault traffic on a single session resembles one active browser tab, not a parallel API client.

#### Scenario: Embedded limits use reduced pacing knobs
- **WHEN** embedded `provider-limits.yaml` is loaded
- **THEN** provider `chatgpt-web` tier `plus-single-session` defines **`rpm: 4`**, **`concurrent: 1`**, and **`min-interval-ms: 12000`**
- **AND** these values replace the previous **`12` / `2` / `3000`** profile

#### Scenario: Pacing gate enforces the new profile
- **WHEN** the pacing registry resolves limits for `chatgpt-web`
- **THEN** the effective gate allows at most **4** paced starts per rolling minute
- **AND** at most **1** concurrent in-flight completion per credential scope
- **AND** enforces at least **12 seconds** between consecutive paced starts

### Requirement: Abuse-block cooldown tier
The gateway SHALL support an `abuse-block` cooldown duration in provider limit configuration, distinct from `provider-error`, `rate-limit`, and `auth-error`.

#### Scenario: chatgpt-web defines long abuse-block cooldown
- **WHEN** embedded `provider-limits.yaml` is loaded
- **THEN** provider `chatgpt-web` defines `cooldown.abuse-block` of **4 hours**
- **AND** global `cooldown-defaults` defines a fallback `abuse-block` duration for providers without an override

### Requirement: Classify OpenAI risk-block responses
The gateway SHALL detect upstream abuse/risk-block messages in response bodies and apply the `abuse-block` cooldown instead of the short `provider-error` cooldown.

#### Scenario: Unusual activity body triggers abuse-block
- **WHEN** a `chatgpt-web` dispatch returns HTTP 502
- **AND** the response body contains OpenAI copy such as “Our systems have detected unusual activity”
- **THEN** the router records failure with cooldown duration **`abuse-block + retry-after-buffer`**
- **AND** cooldown duration is at least **4 hours** for `chatgpt-web` (per provider override)

#### Scenario: Sentinel hard-block triggers abuse-block
- **WHEN** a `chatgpt-web` dispatch returns HTTP 502
- **AND** the response body indicates a sentinel hard block (e.g. contains “Sentinel” and “blocked”)
- **THEN** the router applies the `abuse-block` cooldown
- **AND** does not apply the short `provider-error` cooldown

#### Scenario: Generic 502 without abuse copy keeps provider-error cooldown
- **WHEN** a provider returns HTTP 502 with a generic upstream error message
- **AND** the body does not match abuse-block patterns
- **THEN** the router applies the existing `provider-error` cooldown (60s for `chatgpt-web`)

#### Scenario: Rate limit and auth paths unchanged
- **WHEN** `chatgpt-web` returns HTTP 429 or HTTP 401/403 for session auth without abuse-block body copy
- **THEN** cooldown uses existing `rate-limit` or `auth-error` tiers respectively
- **AND** abuse-block classification does not override those paths

### Requirement: Automated tests without live ChatGPT
The gateway SHALL ship unit tests that validate warmup caching, cache invalidation, cookie rotation, and abuse-block cooldown selection without calling chatgpt.com.

#### Scenario: CI verifies stabilization behavior
- **WHEN** tests run for `chatgpt-web-stabilization`
- **THEN** warmup cache skip/hit and auth-failure invalidation are covered with `MockFetch`
- **AND** session-token rotation tests cover unchunked↔chunked merge with CF cookies preserved
- **AND** abuse phrase classification and `cooldown_for_response` duration assertions pass
- **AND** provider limit catalog parsing includes `chatgpt-web` pacing (**4 / 1 / 12s**) and `abuse-block`

### Requirement: Documentation and release version
The gateway SHALL document ChatGPT Web stabilization (warmup cache, cache invalidation, abuse-block cooldown, operational playbook) and SHALL ship this capability in release **`0.3.0-beta.13`**.

#### Scenario: Contributor reads stabilization docs
- **WHEN** an operator opens `docs/chatgpt-web.md`
- **THEN** the doc explains warmup caching, reduced pacing (4 rpm, 12s spacing, 1 concurrent), long cooldown on unusual activity, browser sanity check on the same IP, and that repeated retries extend upstream blocks
