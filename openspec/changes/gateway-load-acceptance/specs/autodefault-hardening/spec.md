## ADDED Requirements

**Approach:** Stage hardening — eliminate **knowable-before-HTTP** failures. Each requirement
maps to a routing-load scenario that proves the guardrail under concurrent dispatch (see
`routing-load-verification` capability). Production verification uses
`GET /v1/observability/provider-stats` per `(provider, credential)` — not response content.

**Priority:** P0 stops dead hops immediately; P1 fixes failover/parsing; P2 adds observability
parity for web-session chunking.

---

### Requirement: longcat excluded from autodefault until access restored (P0 #1)

The gateway MUST exclude providers without working API access from autodefault candidate lists
when upstream returns `Unsupported model` or sustained auth/model errors.

When such a credential enters access-denied state, the gateway MUST:

1. Omit the slot from autodefault candidates.
2. Apply a credential-slot cooldown of at least **24 hours** before re-eligibility.
3. Remove stale autodefault model aliases until the operator confirms a working model id.

#### Scenario: unsupported-model-zero-attempts

**Given** a provider is configured but upstream returns `Unsupported model`
**When** autodefault ranks candidates
**Then** no candidate for that provider MUST appear in the dispatch chain
**And** zero upstream attempts MUST be made for the session

#### Scenario: twenty-four-hour-cooldown-prevents-repeated-probes

**Given** a slot entered a 24-hour access-denied cooldown
**When** the next autodefault request arrives within the cooldown window
**Then** that provider MUST NOT be attempted
**And** failover MUST proceed to the next ranked provider

---

### Requirement: hard payload pre-flight without best-effort tail (P0 #2)

The router MUST exclude candidates whose effective routing window is smaller than the
estimated payload requirement **before** any upstream HTTP call.

When no API-key provider fits the payload, the router MUST NOT relax the minimum context
requirement and MUST NOT select a largest-window best-effort candidate.

Web-session providers MAY remain eligible when their chunk plan can deliver the payload.

**Routing-load signal:** concurrent fat `json_schema` scenario — TPM/context-limited
API providers show zero attempts; eligible providers receive traffic.

#### Scenario: context-overflow-provider-never-called

**Given** a request requires more tokens than a provider's effective routing window
**When** autodefault filters candidates
**Then** that provider MUST NOT receive an upstream call
**And** MUST NOT return HTTP 400 for maximum context length exceeded from that provider

#### Scenario: oversized-api-providers-skipped-web-session-eligible

**Given** all ranked API-key providers have an effective window below the required minimum
**And** a web-session provider has a valid multi-turn chunk plan
**When** payload filtering completes
**Then** the filtered API-key candidate list MUST be empty
**And** the web-session provider MUST remain eligible

---

### Requirement: chatgpt-web conservative upload chunks (P0 #3)

ChatGPT Web MUST use a per-part upload token cap of **45000 tokens**, matching DeepSeek Web.

Prompts above single-turn budget MUST produce `upload_parts > 1`.

**Routing-load signal:** last-resort scenario with fat dossier — zero HTTP 413.

#### Scenario: large-prompt-multi-turn-upload

**Given** a request with approximately 80000 estimated input tokens
**When** ChatGPT Web plans conversation turns
**Then** upload part count MUST be greater than one

#### Scenario: chatgpt-last-resort-no-message-length-error

**Given** autodefault reaches ChatGPT Web as last resort with a large payload
**When** the chunk plan is executed
**Then** upstream MUST NOT return HTTP 413 message length exceeded

---

### Requirement: github-models response content normalization (P1 #4)

The gateway MUST normalize upstream chat completion JSON so message `content` is a string
before deserializing into OpenAI response types. Streaming chunks MUST receive the same
normalization via the shared `openai_chat_response` normalizer.

#### Scenario: content-array-deserializes

**Given** an upstream returns HTTP 200 with message content as a JSON array
**When** the gateway maps the response
**Then** deserialization MUST succeed without internal error

---

### Requirement: gemini multi-slot overload and rate-limit rotation (P1 #7)

The gateway MUST apply the following sibling policy for multi-slot providers at the same
budget rank:

| Upstream signal | Sibling behaviour |
|-----------------|-------------------|
| HTTP 429 RPM | Try next sibling; failed slot gets Retry-After cooldown only |
| HTTP 503 overload | Try next sibling (Transient class, not Overload skip-all) |
| Daily quota exhausted | Skip remaining same-rank siblings |

**Routing-load signal:** concurrent RPM failover and 503 rotation scenarios with per-credential
mocks — terminal success on sibling, zero browser-session attempts.

#### Scenario: overload-tries-next-free-slot

**Given** multiple free credential slots at the same budget rank
**And** the first slot returns HTTP 503 overload
**When** failover processes the failure
**Then** the next sibling slot MUST be attempted

#### Scenario: daily-quota-skips-same-rank-siblings

**Given** a free slot returns daily quota exhaustion
**When** failover processes the failure
**Then** remaining same-rank siblings MUST be skipped
**And** a paid-tier slot at a different budget rank MUST remain eligible

---

### Requirement: daily quota proactive gate and long cooldown (P1 #8)

Providers with daily allocation MUST be blocked by proactive RPD/TPD gates (see
`catalog-quota-pacing`) when exhausted. Reactive daily quota MUST use provider
`quota-exhausted` cooldown until daily reset.

**Routing-load signal:** daily-quota failover scenario — zero re-hops to exhausted provider
within the same UTC day.

#### Scenario: daily-cap-zero-upstream-hops

**Given** daily allocation is exhausted in pacing state for a provider scope
**When** autodefault evaluates that scope
**Then** no upstream HTTP call MUST be made
**And** failover MUST advance immediately

---

### Requirement: chatgpt-web chunking observability (P2 #9)

The gateway MUST expose ChatGPT Web chunking metrics equivalent to DeepSeek Web via the
`routing-observability` capability (`chatgpt_web_turns`, `chatgpt_web_upload_parts`).

#### Scenario: observability-delegated-to-routing-observability-spec

**Given** a multi-turn ChatGPT Web dispatch completes
**When** provider-stats is queried
**Then** chunking fields MUST be present per the routing-observability requirement

---

### Requirement: openrouter insufficient-credits guard (P2 #10)

The gateway MUST apply the dual-gate rules from `credential-budget-availability` on the
autodefault free-first path and MUST NOT burn hops on repeated HTTP 402 for the same
paid route.

#### Scenario: openrouter-guard-delegated-to-budget-availability

**Given** OpenRouter reports zero remaining credits for a paid model route
**When** autodefault evaluates the slot
**Then** behaviour MUST match the credential-budget-availability paid-path block scenario
