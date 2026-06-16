# DeepSeek Web provider

The `deepseek-web` provider routes chat completions through a **browser session**
(`localStorage.userToken` on chat.deepseek.com), not the DeepSeek API key.

## Enable

1. Set session file path in `.env`:

   ```bash
   DEEPSEEK_BROWSER_CLI=dev/deepseek-session.json
   ```

   Or use the credential slot:

   ```bash
   AI_GATEWAY_CREDENTIAL_DEEPSEEK_WEB_DEFAULT=dev/deepseek-session.json
   ```

2. Create the session (requires `deepseek-login` feature):

   **Interactive browser login:**

   ```bash
   cargo run --features deepseek-login -- deepseek login
   ```

   **Import token from DevTools:**

   ```bash
   cargo run --features deepseek-login -- deepseek import \
     --token 'your-userToken-value'
   ```

   Token location: DevTools → Application → Local Storage →
   `chat.deepseek.com` → `userToken` (plain string or JSON `{"value":"..."}`).

3. Restart the gateway. When the session file exists, `deepseek-web` joins
   **autodefault** with high priority (after `chatgpt-web` when both are configured).

## Smoke test

Verify token exchange without starting the server:

```bash
DEEPSEEK_BROWSER_CLI=dev/deepseek-session.json \
  cargo run --features deepseek-login -- deepseek probe
```

Optional completion smoke:

```bash
DEEPSEEK_BROWSER_CLI=dev/deepseek-session.json \
  cargo run --features deepseek-login -- deepseek probe \
  --query 'Reply with exactly one word: OK'
```

Manual curl (token exchange only):

```bash
TOKEN=$(jq -r .token dev/deepseek-session.json)
curl -sS "https://chat.deepseek.com/api/v0/users/current" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Origin: https://chat.deepseek.com"
```

## Models

Embedded catalog ([`providers.yaml`](../ai-gateway/config/embedded/providers.yaml)):

| Model | Notes |
|-------|-------|
| `deepseek-web/deepseek-chat` | Standard chat |
| `deepseek-web/deepseek-reasoner` | Thinking enabled (`reasoning_content`) |

Capabilities: tools **not** supported initially (`supports-tools: false`).

## Session maintenance

Sessions expire when DeepSeek invalidates `userToken`. Symptoms: HTTP 401,
auth-error cooldown on the credential. Re-run `deepseek login` or `deepseek import`.

## Pacing

Conservative single-session limits in
[`provider-limits.yaml`](../ai-gateway/config/embedded/provider-limits.yaml):

| Knob | Value |
|------|-------|
| RPM | **6** |
| Concurrent | **1** |
| Min interval | **10s** |
| Rate-limit cooldown | **120s** |
| Auth-error cooldown | **30m** |

Each completion performs several upstream calls (token exchange, session create,
PoW challenge, completion). Pacing limits **completion starts**, not raw HTTP.

## Related

- [credentials.md](credentials.md)
- [providers.md](providers.md)
- [chatgpt-web.md](chatgpt-web.md)
