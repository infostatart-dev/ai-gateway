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

## Security

- Treat `session.json` like a password — add to `.gitignore` (see repo
  `.gitignore`).
- Do not commit session files or paste cookies into logs.

## Related

- [providers.md](providers.md)
- [routing.md](routing.md)
- [credentials.md](credentials.md)
