# ChatGPT Web provider

The `chatgpt-web` provider routes chat completions through a **browser session**
(ChatGPT web UI cookies), not an OpenAI API key.

## Enable

1. Set session file path in `.env`:

   ```bash
   CHATGPT_BROWSER_CLI=dev/session.json
   ```

2. Create the session (requires `chatgpt-login` feature):

   **Interactive browser login:**

   ```bash
   cargo run --features chatgpt-login -- chatgpt login
   ```

   **Import cookies from DevTools:**

   ```bash
   cargo run --features chatgpt-login -- chatgpt import \
     --cookie '__Secure-next-auth.session-token=...; cf_clearance=...'
   ```

   The session is saved to the path in `CHATGPT_BROWSER_CLI`.

3. Restart the gateway. If the session file exists, `chatgpt-web` joins the
   **autodefault** router with highest priority.

## Model

Embedded catalog ([`providers.yaml`](../ai-gateway/config/embedded/providers.yaml)):

```
chatgpt-web/gpt-5.5-instant
```

Capabilities: JSON schema supported; tools not supported.

## JSON schema requests

Strict `response_format.type = json_schema` requests are validated against
provider capabilities. ChatGPT Web is selected when it supports the requested
schema constraints and session is available.

## Session maintenance

Sessions expire when ChatGPT invalidates cookies. Symptoms: auth errors,
cooldown on the chatgpt-web credential. Re-run `chatgpt login` or `chatgpt
import`.

Provider limits and cooldown entries for chatgpt-web are in
[`provider-limits.yaml`](../ai-gateway/config/embedded/provider-limits.yaml).

## Stabilization (beta.13+)

### Warmup cache

Before sentinel, the executor performs three browser-like GETs (`/me`,
`/conversations`, `/models`). Results are cached for **60 seconds** per session
cookie + access-token suffix so burst traffic does not repeat warmup on every
completion.

Caches are invalidated on HTTP **401/403** from session exchange, sentinel, or
conversation so a blocked session does not skip warmup on the next attempt.

### Pacing

Embedded pacing for tier `plus-single-session` targets one active browser tab:

| Knob | Value |
|------|-------|
| RPM | **4** |
| Concurrent | **1** |
| Min interval | **12s** |

### Cooldowns

| Kind | `chatgpt-web` override |
|------|------------------------|
| Rate limit (429) | **180s** |
| Auth error | **30m** |
| Provider error (generic 502) | **60s** |
| **Abuse block** (unusual activity / sentinel hard block) | **4h** |

When upstream returns **502** or **503** with OpenAI “unusual activity” copy or
sentinel block messages, the router applies **`abuse-block`** instead of the
short provider-error cooldown.

### Operational playbook

1. **Browser sanity check** — if chatgpt.com shows “unusual activity” in a normal
   browser on the same egress IP, stop gateway retries for hours; cooldown alone
   is not enough until the IP clears.
2. **Do not hammer** — repeated autodefault failover that re-selects `chatgpt-web`
   every minute extends blocks; `abuse-block` cooldown prevents retry storms.
3. **Full cookie jar** — import/login must include session token **and**
   Cloudflare cookies (`cf_clearance`, `__cf_bm`); bare token-only paste fails
   sentinel/CF more often.
4. **Egress** — datacenter/pod IPs are high-risk; residential or dedicated egress
   per session reduces false positives (ops concern).
5. **One session per account** — sharing one session file across many replicas on
   one IP multiplies automated traffic patterns.
6. **Recovery window** — expect **1–24h** for light flags; stop all attempts during
   wait.

## Security

- Treat `session.json` like a password — add to `.gitignore` (see repo
  `.gitignore`).
- Do not commit session files or paste cookies into logs.

## Related

- [providers.md](providers.md)
- [routing.md](routing.md)
- [credentials.md](credentials.md)
