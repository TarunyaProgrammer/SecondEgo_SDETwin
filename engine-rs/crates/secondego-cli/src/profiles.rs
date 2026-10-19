//! Terminal provider profiles backed by the native OS credential vault.
//!
//! Secrets are accepted only from stdin and are never written to the profile
//! metadata file. The metadata path intentionally matches Electron's userData
//! location on macOS/Linux/Windows so desktop and terminal share profile
//! selection.

use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use secondego_model::ProviderKind;
use serde_json::{Value, json};

#[path = "windows_vault.rs"]
mod windows_vault;

const KEYCHAIN_SERVICE: &str = "com.secondego.provider";

#[derive(Debug)]
pub enum ProfileError {
    Invalid(String),
    Io(String),
    Metadata(String),
    Keychain(String),
}

impl std::fmt::Display for ProfileError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(message)
            | Self::Io(message)
            | Self::Metadata(message)
            | Self::Keychain(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for ProfileError {}

#[derive(Debug, Clone)]
struct ProfileEntry {
    provider: String,
    profile: String,
    model: Option<String>,
}

fn provider(value: &str) -> Result<ProviderKind, ProfileError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "deepseek" => Ok(ProviderKind::DeepSeek),
        "gemini" => Ok(ProviderKind::Gemini),
        "openai" => Ok(ProviderKind::OpenAI),
        "anthropic" => Ok(ProviderKind::Anthropic),
        "openrouter" => Ok(ProviderKind::OpenRouter),
        "groq" => Ok(ProviderKind::Groq),
        _ => Err(ProfileError::Invalid(
            "provider must be deepseek, gemini, openai, anthropic, openrouter, or groq".into(),
        )),
    }
}

fn safe_profile(value: &str) -> Result<String, ProfileError> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 96
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        })
    {
        return Err(ProfileError::Invalid(
            "profile name must contain only letters, numbers, '.', '_' or '-'".into(),
        ));
    }
    Ok(value.into())
}

fn profile_id(provider: ProviderKind, profile: &str) -> String {
    format!("{}:{profile}", provider.name())
}

fn metadata_path() -> Result<PathBuf, ProfileError> {
    let path = if cfg!(target_os = "windows") {
        let app_data = env::var_os("APPDATA")
            .or_else(|| env::var_os("USERPROFILE"))
            .map(PathBuf::from)
            .ok_or_else(|| {
                ProfileError::Io(
                    "APPDATA or USERPROFILE is not set; cannot locate provider profiles".into(),
                )
            })?;
        app_data.join("SecondEgo/profiles.json")
    } else {
        let home = env::var_os("HOME").map(PathBuf::from).ok_or_else(|| {
            ProfileError::Io("HOME is not set; cannot locate provider profiles".into())
        })?;
        if cfg!(target_os = "macos") {
            home.join("Library/Application Support/SecondEgo/profiles.json")
        } else {
            home.join(".config/SecondEgo/profiles.json")
        }
    };
    Ok(path)
}

fn read_metadata() -> Result<Value, ProfileError> {
    let path = metadata_path()?;
    if !path.is_file() {
        return Ok(json!({"active": null, "entries": {}}));
    }
    let text = fs::read_to_string(path).map_err(|error| ProfileError::Io(error.to_string()))?;
    serde_json::from_str(&text).map_err(|error| ProfileError::Metadata(error.to_string()))
}

fn write_metadata(value: &Value) -> Result<(), ProfileError> {
    let path = metadata_path()?;
    let parent = path
        .parent()
        .ok_or_else(|| ProfileError::Io("profile metadata has no parent directory".into()))?;
    fs::create_dir_all(parent).map_err(|error| ProfileError::Io(error.to_string()))?;
    let temporary = parent.join(format!("profiles.json.{}.tmp", std::process::id()));
    fs::write(
        &temporary,
        format!(
            "{}\n",
            serde_json::to_string_pretty(value).unwrap_or_default()
        ),
    )
    .map_err(|error| ProfileError::Io(error.to_string()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600))
            .map_err(|error| ProfileError::Io(error.to_string()))?;
    }
    fs::rename(temporary, path).map_err(|error| ProfileError::Io(error.to_string()))
}

fn entry_from_metadata(value: &Value, id: &str) -> Option<ProfileEntry> {
    let item = value.get("entries")?.get(id)?;
    Some(ProfileEntry {
        provider: item.get("provider")?.as_str()?.into(),
        profile: item.get("profile")?.as_str()?.into(),
        model: item.get("model").and_then(Value::as_str).map(str::to_owned),
    })
}

fn native_command(
    entry: &ProfileEntry,
    action: &str,
    secret: Option<&str>,
) -> Result<String, ProfileError> {
    let provider = provider(&entry.provider)?;
    let account = profile_id(provider, &entry.profile);
    if cfg!(target_os = "windows") {
        return windows_vault::credential(action, &account, secret);
    }
    let (command, mut arguments) = if cfg!(target_os = "macos") {
        let arguments = match action {
            "save" => vec![
                "add-generic-password".into(),
                "-U".into(),
                "-a".into(),
                account.clone(),
                "-s".into(),
                KEYCHAIN_SERVICE.into(),
            ],
            "read" => vec![
                "find-generic-password".into(),
                "-a".into(),
                account.clone(),
                "-s".into(),
                KEYCHAIN_SERVICE.into(),
                "-w".into(),
            ],
            "delete" => vec![
                "delete-generic-password".into(),
                "-a".into(),
                account.clone(),
                "-s".into(),
                KEYCHAIN_SERVICE.into(),
            ],
            _ => return Err(ProfileError::Invalid("unknown keychain action".into())),
        };
        ("security", arguments)
    } else {
        let arguments = match action {
            "save" => vec![
                "store".into(),
                "--label=SecondEgo provider key".into(),
                "service".into(),
                KEYCHAIN_SERVICE.into(),
                "account".into(),
                account.clone(),
            ],
            "read" => vec![
                "lookup".into(),
                "service".into(),
                KEYCHAIN_SERVICE.into(),
                "account".into(),
                account.clone(),
            ],
            "delete" => vec![
                "clear".into(),
                "service".into(),
                KEYCHAIN_SERVICE.into(),
                "account".into(),
                account.clone(),
            ],
            _ => return Err(ProfileError::Invalid("unknown keychain action".into())),
        };
        ("secret-tool", arguments)
    };
    if cfg!(target_os = "macos") && action == "save" {
        // With no value after -w, security prompts through stdin. This keeps
        // the key out of the child process argument list.
        arguments.push("-w".into());
    }
    let mut child = Command::new(command)
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| {
            ProfileError::Keychain("the native credential-vault command is unavailable".into())
        })?;
    if let Some(secret) = secret {
        use std::io::Write;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(format!("{secret}\n").as_bytes())
                .map_err(|_| {
                    ProfileError::Keychain("could not send the key to the credential vault".into())
                })?;
        }
    }
    let output = child
        .wait_with_output()
        .map_err(|_| ProfileError::Keychain("credential-vault request failed".into()))?;
    if !output.status.success() {
        return Err(ProfileError::Keychain(
            "credential-vault request was rejected".into(),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().into())
}

fn active_entry(value: &Value) -> Option<ProfileEntry> {
    let id = value.get("active")?.as_str()?;
    entry_from_metadata(value, id)
}

pub fn load_active_into_environment() -> Result<bool, ProfileError> {
    let metadata = read_metadata()?;
    let Some(entry) = active_entry(&metadata) else {
        return Ok(false);
    };
    let kind = provider(&entry.provider)?;
    // An explicitly supplied provider key is the automation escape hatch and
    // must win over a stored interactive profile. This keeps CI and one-off
    // shell invocations deterministic instead of silently selecting a local
    // profile from the user's machine.
    if let Some(explicit_provider) = env::var("SECONDEGO_PROVIDER")
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        if !explicit_provider.eq_ignore_ascii_case(kind.name()) {
            return Ok(false);
        }
        if env::var(kind.credential_env_name())
            .ok()
            .is_some_and(|value| !value.trim().is_empty())
        {
            return Ok(false);
        }
    }
    let key = native_command(&entry, "read", None)?;
    if key.is_empty() {
        return Ok(false);
    }
    unsafe {
        env::set_var("SECONDEGO_PROFILE_ID", profile_id(kind, &entry.profile));
        env::set_var("SECONDEGO_PROVIDER", kind.name());
        if let Some(model) = entry.model.as_deref().filter(|model| !model.is_empty()) {
            env::set_var("SECONDEGO_MODEL", model);
        }
        env::set_var(kind.credential_env_name(), key);
    }
    Ok(true)
}

/// Return safe provider-level readiness from native profile metadata.
///
/// The vault value is read only long enough to determine whether it exists;
/// this function never returns or prints the secret.
pub fn provider_vault_status() -> Result<Vec<(ProviderKind, bool)>, ProfileError> {
    let metadata = read_metadata()?;
    let entries = metadata
        .get("entries")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut statuses: Vec<(ProviderKind, bool)> = Vec::new();
    for id in entries.keys() {
        let Some(entry) = entry_from_metadata(&metadata, id) else {
            continue;
        };
        let kind = provider(&entry.provider)?;
        let ready = native_command(&entry, "read", None)
            .map(|value| !value.is_empty())
            .unwrap_or(false);
        if let Some((_, existing_ready)) = statuses.iter_mut().find(|(item, _)| *item == kind) {
            *existing_ready |= ready;
        } else {
            statuses.push((kind, ready));
        }
    }
    Ok(statuses)
}

fn option_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
}

fn require_key_from_stdin() -> Result<String, ProfileError> {
    let mut value = String::new();
    io::stdin()
        .read_to_string(&mut value)
        .map_err(|error| ProfileError::Io(error.to_string()))?;
    let value = value.trim().to_owned();
    if value.len() < 8 || value.len() > 512 {
        return Err(ProfileError::Invalid(
            "stdin key must be between 8 and 512 characters".into(),
        ));
    }
    Ok(value)
}

pub fn run_command(args: &[String]) -> Result<(), ProfileError> {
    let command = args.first().map(String::as_str).unwrap_or("list");
    match command {
        "list" => {
            let metadata = read_metadata()?;
            let active = metadata.get("active").and_then(Value::as_str);
            let entries = metadata
                .get("entries")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            if entries.is_empty() {
                println!("No native provider profiles saved. Add one with --key-stdin.");
                return Ok(());
            }
            for (id, item) in entries {
                let entry = entry_from_metadata(&metadata, &id).ok_or_else(|| {
                    ProfileError::Metadata("provider profile metadata is malformed".into())
                })?;
                let ready = native_command(&entry, "read", None).is_ok();
                println!(
                    "{} {} / {} — {}{}",
                    if active == Some(id.as_str()) {
                        "*"
                    } else {
                        " "
                    },
                    entry.provider,
                    entry.profile,
                    if ready { "READY" } else { "MISSING" },
                    entry
                        .model
                        .as_deref()
                        .map(|model| format!(" model={model}"))
                        .unwrap_or_default()
                );
                let _ = item;
            }
            Ok(())
        }
        "add" => {
            let provider_kind = provider(args.get(1).map(String::as_str).unwrap_or_default())?;
            if !args.iter().any(|argument| argument == "--key-stdin") {
                return Err(ProfileError::Invalid(
                    "provider add requires --key-stdin so the key never appears in shell history"
                        .into(),
                ));
            }
            let profile =
                safe_profile(&option_value(args, "--profile").unwrap_or_else(|| "default".into()))?;
            let model = option_value(args, "--model");
            let entry = ProfileEntry {
                provider: provider_kind.name().into(),
                profile: profile.clone(),
                model: model.clone(),
            };
            let key = require_key_from_stdin()?;
            native_command(&entry, "save", Some(&key))?;
            let mut metadata = read_metadata()?;
            let id = profile_id(provider_kind, &profile);
            let entries = metadata
                .get_mut("entries")
                .and_then(Value::as_object_mut)
                .ok_or_else(|| {
                    ProfileError::Metadata("provider profile metadata is malformed".into())
                })?;
            entries.insert(
                id.clone(),
                json!({"provider": provider_kind.name(), "profile": profile, "model": model, "updatedAt": "terminal"}),
            );
            if metadata.get("active").is_none_or(Value::is_null) {
                metadata["active"] = Value::String(id);
            }
            write_metadata(&metadata)?;
            println!("Saved native provider profile; key value was not printed.");
            Ok(())
        }
        "use" => {
            let provider_kind = provider(args.get(1).map(String::as_str).unwrap_or_default())?;
            let profile =
                safe_profile(&option_value(args, "--profile").unwrap_or_else(|| "default".into()))?;
            let entry = ProfileEntry {
                provider: provider_kind.name().into(),
                profile: profile.clone(),
                model: None,
            };
            native_command(&entry, "read", None)?;
            let mut metadata = read_metadata()?;
            let id = profile_id(provider_kind, &profile);
            if entry_from_metadata(&metadata, &id).is_none() {
                return Err(ProfileError::Invalid(
                    "profile metadata does not contain that profile".into(),
                ));
            }
            metadata["active"] = Value::String(id);
            write_metadata(&metadata)?;
            println!(
                "Activated provider profile {} / {}.",
                provider_kind.name(),
                profile
            );
            Ok(())
        }
        "remove" => {
            let provider_kind = provider(args.get(1).map(String::as_str).unwrap_or_default())?;
            let profile =
                safe_profile(&option_value(args, "--profile").unwrap_or_else(|| "default".into()))?;
            let entry = ProfileEntry {
                provider: provider_kind.name().into(),
                profile: profile.clone(),
                model: None,
            };
            native_command(&entry, "delete", None)?;
            let mut metadata = read_metadata()?;
            let id = profile_id(provider_kind, &profile);
            metadata
                .get_mut("entries")
                .and_then(Value::as_object_mut)
                .map(|entries| entries.remove(&id));
            if metadata.get("active").and_then(Value::as_str) == Some(id.as_str()) {
                metadata["active"] = Value::Null;
            }
            write_metadata(&metadata)?;
            println!(
                "Removed provider profile {} / {}.",
                provider_kind.name(),
                profile
            );
            Ok(())
        }
        "help" => {
            println!("Usage:");
            println!(
                "  secondego provider add PROVIDER --key-stdin [--profile NAME] [--model MODEL]"
            );
            println!("  secondego provider use PROVIDER [--profile NAME]");
            println!("  secondego provider list");
            println!("  secondego provider remove PROVIDER [--profile NAME]");
            Ok(())
        }
        _ => Err(ProfileError::Invalid(
            "use provider add, use, list, or remove".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_names_are_shell_safe() {
        assert_eq!(safe_profile("work-laptop").unwrap(), "work-laptop");
        assert!(safe_profile("../secrets").is_err());
        assert!(safe_profile("provider profile").is_err());
    }

    #[test]
    fn metadata_ids_are_provider_scoped() {
        assert_eq!(
            profile_id(ProviderKind::OpenRouter, "default"),
            "openrouter:default"
        );
        assert!(provider("not-a-provider").is_err());
    }
}
