# Provider profiles

SecondEgo supports five required provider profiles: DeepSeek, Gemini, OpenAI,
Anthropic, and OpenRouter. Groq remains available as an optional provider.

## Credential names

| Provider | Environment variable | Default model |
| --- | --- | --- |
| DeepSeek | `DEEPSEEK_API_KEY` | `deepseek-chat` |
| Gemini | `GEMINI_API_KEY` | `gemini-3.8-flash` |
| OpenAI | `OPENAI_API_KEY` | `gpt-5-mini` |
| Anthropic | `ANTHROPIC_API_KEY` | `claude-sonnet-4-6` |
| OpenRouter | `OPENROUTER_API_KEY` | `openai/gpt-5-mini` |

`AI_API_KEY` is retained as a migration-only alias. An explicitly named key
always wins, and using the alias emits a warning.

## Terminal

Use stdin so the key does not appear in shell history:

```bash
printf '%s\n' "$OPENAI_API_KEY" \
  | secondego provider add openai --key-stdin --profile work --model gpt-5-mini
secondego provider list
secondego provider use openai --profile work
secondego provider test
secondego provider list --json
```

On macOS, secrets are stored in Keychain. On Linux, secrets are stored in
Secret Service/libsecret through `secret-tool`. `profiles.json` contains only
provider/profile/model metadata and the active profile id. It never contains a
secret. `provider remove` deletes both the vault item and its metadata.

The packaged macOS desktop app includes a small native
`SecondEgoCredentialHelper` built against Security.framework. Electron sends
save/read/delete operations to that helper over stdin/stdout, so a provider key
does not appear in the `security` command's process arguments. Development
builds fall back to the system `security` command when the helper has not been
compiled; release builds stage the helper automatically.

The terminal adapter currently supports macOS and Linux. Windows users should
use the desktop vault or provider-specific environment variables until the
Credential Manager adapter lands.

## Desktop

Open `Settings → Providers`, choose a provider, enter a profile name and model,
save the key, and run `Test connection`. The renderer does not keep the key in
React state. Electron sends it directly to the main process, which writes to
the operating system vault. Only the selected profile is resolved for a local
run.

## Security boundary

Keys are not written to SQLite, logs, diffs, telemetry, prompts, or profile
metadata. The current local engine boundary injects the selected key into the
short-lived engine/gateway process environment because the provider adapters
consume the existing environment contract. The next hardening step is a
credential-handle or in-process provider session so the key never crosses a
child-process environment boundary.

## Failure handling

`provider test` performs a bounded authenticated request and reports only the
provider, model, duration, or a classified failure. It does not print the key,
request body, response body, or raw provider error text.

`provider list --json` and `provider test --json` emit schema-versioned,
machine-readable output for CI and shell integrations. They contain provider
ids, models, profile ids, and safe status values only.
