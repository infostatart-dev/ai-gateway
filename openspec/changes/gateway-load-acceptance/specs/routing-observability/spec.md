## MODIFIED Requirements

**Approach:** Provider observability is the **primary routing verification surface**
(routing-load-verification D1). Assert per `(provider, credential)` row in
`GET /v1/observability/provider-stats` — not stub call counts or LLM content quality.

Web-session providers MUST expose chunking parity so last-resort fat-payload paths are
debuggable without reproducing upstream 413 failures.

---

### Requirement: web-session chunking metrics

Provider observability MUST record chunking statistics for web-session providers.

| Provider | Fields |
|----------|--------|
| DeepSeek Web | `deepseek_web_turns`, `deepseek_web_upload_parts` |
| ChatGPT Web | `chatgpt_web_turns`, `chatgpt_web_upload_parts` |

All four fields MUST appear in dispatch trace and in `GET /v1/observability/provider-stats`
route summaries when the respective provider handled the request.

#### Scenario: chatgpt-fields-match-deepseek-convention

**Given** a multi-turn ChatGPT Web dispatch completes
**When** provider metrics are recorded
**Then** `chatgpt_web_turns` and `chatgpt_web_upload_parts` MUST be present
**And** MUST follow the same naming convention as DeepSeek Web fields

#### Scenario: provider-stats-row-per-credential

**Given** multiple credential slots for one provider received attempts
**When** provider-stats is queried
**Then** each slot MUST appear as a distinct row keyed by provider and credential id
**And** attempt totals MUST be independently assertable per slot
