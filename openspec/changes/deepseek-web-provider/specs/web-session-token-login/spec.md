## ADDED Requirements

### Requirement: Headed-browser login can capture a page-scoped token

The shared browser-login flow SHALL support capturing a page-scoped credential
value (such as a `localStorage` entry) in addition to cookies, so a web provider
whose credential is a bearer token rather than a cookie can be authenticated
through the same headed-browser flow.

#### Scenario: Capture localStorage token after login

- **WHEN** an operator completes login in the headed browser on a provider page that stores its credential in `localStorage`
- **THEN** the login flow reads the configured `localStorage` key from the page and returns its value once present

#### Scenario: Empty localStorage does not abort the poll

- **WHEN** the headed browser is open but `localStorage` for the configured key is still empty or null
- **THEN** the poll loop continues waiting until a non-empty value appears or timeout
- **AND** does not exit with an error solely because the value is missing

#### Scenario: Cookie capture unchanged for existing providers

- **WHEN** an existing cookie-based provider (chatgpt-web, perplexity-web) runs its login flow
- **THEN** cookie capture behaves exactly as before, with no token extraction required

### Requirement: DeepSeek login persists a session file

The `deepseek login` command SHALL persist the captured `userToken` into a
session file and SHALL provide a manual import fallback for environments where
headed-browser login is unavailable.

#### Scenario: Successful browser login writes session file

- **WHEN** `deepseek login` captures a `userToken` from chat.deepseek.com
- **THEN** a session file containing the token is written at the configured path (`DEEPSEEK_BROWSER_CLI`)

#### Scenario: Manual token import fallback

- **WHEN** an operator provides a `userToken` via `deepseek import --token`
- **THEN** the command writes the equivalent session file without launching a browser
- **AND** normalizes JSON-wrapped tokens `{"value":"..."}` to the inner string
