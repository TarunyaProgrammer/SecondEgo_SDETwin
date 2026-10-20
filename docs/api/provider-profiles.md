# Provider profiles

SecondEgo supports five required provider profiles: DeepSeek, Gemini, OpenAI,
Anthropic, and OpenRouter. Groq remains available as an optional provider.

## Credential names

| Provider | Environment variable | Default model |
| --- | --- | --- |
| DeepSeek | `DEEPSEEK_API_KEY` | `deepseek-flash` |
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
  | secondego provider profile add openai --key-stdin --profile work --model gpt-5-mini
secondego provider profile list
secondego provider profile use openai --profile work
secondego provider test
secondego provider models
secondego provider profile list --json
```

`provider list` reports provider-level readiness. `provider profile list` reports
the saved profile metadata and active profile as a separate stable contract:

```json
{
  "schema_version": 1,
  "active_profile": "openai:work",
  "profiles": [
    {
      "id": "openai:work",
      "provider": "openai",
      "profile": "work",
      "model": "gpt-5-mini",
      "active": true,
      "ready": true
    }
  ]
}
```

The profile JSON contract never contains a key or vault value. Use it from
launchers, shell integrations, or the desktop bridge when they need to render
profile selection without parsing human-oriented output.

On macOS, secrets are stored in Keychain. On Windows, terminal profiles use
Windows Credential Manager. On Linux, secrets are stored in Secret
Service/libsecret through `secret-tool`. `profiles.json` contains only
provider/profile/model metadata and the active profile id. It never contains a
secret. `provider remove` deletes both the vault item and its metadata.

The packaged macOS desktop app includes a small native
`SecondEgoCredentialHelper` built against Security.framework. Electron sends
save/read/delete operations to that helper over stdin/stdout, so a provider key
does not appear in the `security` command's process arguments. Development
builds fall back to the system `security` command when the helper has not been
compiled; release builds stage the helper automatically.

The terminal adapter supports macOS Keychain, Windows Credential Manager, and
Linux Secret Service. On Windows, the key is written through the native
Credential Manager API rather than `cmdkey`, so it never appears in a command
argument.

## Desktop

Open `Settings → Providers`, choose a provider, enter a profile name and model,
save the key, and run `Test connection`. The renderer does not keep the key in
React state. Electron sends it directly to the main process, which writes to
the operating system vault. Only the selected profile is resolved for a local
run.

In the Rust desktop path, Electron's main process sends that key once to the
authenticated loopback `POST /api/providers/session` endpoint. The renderer
never receives it and the gateway response contains metadata only. The Python
compatibility path and terminal continue to support the named-key environment
contract during migration.

## Security boundary

Keys are not written to SQLite, logs, diffs, telemetry, prompts, or profile
metadata. The desktop Rust gateway receives the selected key through an
authenticated loopback session and retains it only in process memory. The
terminal and Python compatibility path still use named environment variables
as an explicit migration contract; unrelated provider variables are removed
from a desktop Rust child process.

## Failure handling

`provider test` performs a bounded authenticated request and reports only the
provider, model, duration, or a classified failure. It does not print the key,
request body, response body, or raw provider error text.

`provider models` performs authenticated model discovery for the active profile.
The catalog prefers provider capability metadata and removes known embedding,
image, audio, moderation, and reranking-only entries. A custom model override
is still allowed because provider catalogs can lag newly released ids; `Test
connection` remains the authoritative validation step.
`provider list --json`, `provider models --json`, and `provider test --json` emit
schema-versioned, machine-readable output for CI and shell integrations. They
contain provider ids, model ids, profile ids, and safe status values only.
`provider list` reports readiness from either an explicitly supplied environment
key or a saved native-vault profile; it never prints the credential value.
