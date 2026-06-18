## MODIFIED Requirements

**Serves:** autodefault-hardening item 9 — observability for web-session chunking parity.

---

### Requirement: chatgpt-web chunking metrics

ChatGPT Web dispatch MUST record `chatgpt_web_turns` and `chatgpt_web_upload_parts` using
the same pattern as DeepSeek Web. Values MUST appear in `GET /v1/observability/provider-stats`
per credential slot.

#### Scenario: provider-stats-after-multi-part-upload

**Given** a ChatGPT Web dispatch with more than one upload part
**When** provider-stats is queried
**Then** the slot summary MUST include non-zero `chatgpt_web_upload_parts`
