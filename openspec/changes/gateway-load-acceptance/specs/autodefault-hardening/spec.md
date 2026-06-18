## ADDED Requirements

Production autodefault guardrails. Each requirement maps to a **gateway code change**;
proof is via `routing_load` scenarios (see `routing-load-verification` delta).

---

### Requirement: access-denied provider governance

The gateway MUST omit credential slots from autodefault when upstream returns sustained
`Unsupported model` or auth failures. The slot MUST enter a cooldown of at least 24 hours
before re-eligibility. Stale autodefault model aliases MUST be removed until a working
model id is confirmed in the catalog.

#### Scenario: zero-attempts-while-access-denied

**Given** a provider slot is in access-denied cooldown
**When** autodefault ranks candidates
**Then** that slot MUST NOT receive upstream attempts

---

### Requirement: hard payload pre-flight

When no API-key provider's effective window fits the estimated payload, the router MUST NOT
relax requirements or pick a best-effort largest-window candidate. Web-session providers
MAY remain when their chunk plan fits.

#### Scenario: context-overflow-never-dispatched

**Given** estimated tokens exceed a provider's effective window
**When** payload filtering runs
**Then** that API-key provider MUST NOT be dispatched
**And** MUST NOT return HTTP 400 for context length exceeded from that provider

---

### Requirement: chatgpt-web upload chunk parity

ChatGPT Web MUST use a 45000-token per-part upload cap (same as DeepSeek Web). Large
prompts MUST produce multiple upload parts.

#### Scenario: no-413-on-fat-last-resort

**Given** autodefault reaches ChatGPT Web with a large payload
**When** the chunk plan executes
**Then** upstream MUST NOT return HTTP 413 message length exceeded

---

### Requirement: github-models response normalization

GitHub Models upstream JSON MUST be normalized (`content` array → string) before
OpenAI-compatible deserialization, for both streaming and non-streaming paths.

#### Scenario: array-content-deserializes

**Given** GitHub Models returns HTTP 200 with array-shaped message content
**When** the mapper processes the response
**Then** deserialization MUST succeed without internal mapper error

---

### Requirement: gemini failover class split

The gateway MUST classify Gemini upstream failures into distinct failover classes:

| Signal | Behaviour |
|--------|-----------|
| HTTP 429 RPM | Transient — try next sibling at same budget rank |
| HTTP 503 overload | Transient — try next sibling (not skip-all-free) |
| Daily quota exhausted | QuotaExhausted — skip same-provider same-rank siblings |

#### Scenario: overload-rotates-to-sibling

**Given** the first of multiple free-tier sibling slots returns HTTP 503 overload
**When** failover runs
**Then** the next sibling at the same rank MUST be attempted

#### Scenario: daily-quota-skips-siblings-only

**Given** a free-tier slot returns daily quota exhaustion
**When** failover runs
**Then** remaining same-rank siblings MUST be skipped
**And** a paid-tier slot at a different budget rank MUST remain eligible

---

### Requirement: openrouter paid-path guard

When budget probe reports zero remaining credits, paid-model routes MUST be skipped
pre-dispatch. HTTP 402 MUST trigger probe refresh, slot cooldown, and failover to
`:free` variant or next provider — not repeated 402 on the same paid route.

Free-tier `:free` routes MUST remain dispatchable under catalog RPM/RPD when pacing permits.

#### Scenario: zero-credits-paid-skipped-free-allowed

**Given** zero remaining credits and a `:free` model within catalog limits
**When** autodefault selects a candidate
**Then** the paid route MUST be skipped
**And** the free route MAY be selected when pacing permits
