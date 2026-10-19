const { app, safeStorage } = require("electron");
const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");

const PROVIDERS = ["deepseek", "gemini", "openai", "anthropic", "openrouter", "groq"];
const KEY_ENV = {
  deepseek: "DEEPSEEK_API_KEY",
  gemini: "GEMINI_API_KEY",
  openai: "OPENAI_API_KEY",
  anthropic: "ANTHROPIC_API_KEY",
  openrouter: "OPENROUTER_API_KEY",
  groq: "GROQ_API_KEY",
};
const KEYCHAIN_SERVICE = "com.secondego.provider";

function assertSafeValue(value, label, max = 96) {
  if (typeof value !== "string" || !/^[a-z0-9._-]+$/i.test(value) || value.length < 1 || value.length > max) {
    throw new Error(`${label} contains unsupported characters`);
  }
  return value;
}

function assertProvider(provider) {
  if (!PROVIDERS.includes(provider)) throw new Error("Unsupported provider");
  return provider;
}

function storagePath(name) {
  return path.join(app.getPath("userData"), name);
}

function readJson(name, fallback) {
  try {
    const value = JSON.parse(fs.readFileSync(storagePath(name), "utf8"));
    return value && typeof value === "object" ? value : fallback;
  } catch {
    return fallback;
  }
}

function writeJson(name, value) {
  const target = storagePath(name);
  const temporary = `${target}.${process.pid}.tmp`;
  fs.mkdirSync(path.dirname(target), { recursive: true, mode: 0o700 });
  fs.writeFileSync(temporary, `${JSON.stringify(value, null, 2)}\n`, { encoding: "utf8", mode: 0o600 });
  fs.chmodSync(temporary, 0o600);
  fs.renameSync(temporary, target);
}

function ensureSecureStorage() {
  if (nativeBackend()) return;
  if (!safeStorage.isEncryptionAvailable()) {
    throw new Error("The operating system credential vault is not ready; restart SecondEgo and try again.");
  }
  if (typeof safeStorage.getSelectedStorageBackend === "function" && safeStorage.getSelectedStorageBackend() === "basic_text") {
    throw new Error("SecondEgo refused an unencrypted credential backend. Enable the OS keychain and retry.");
  }
}

function nativeBackend() {
  if (process.platform === "darwin") return nativeHelperPath() ? "macos-keychain-helper" : "macos-keychain";
  if (process.platform === "linux" && spawnSync("secret-tool", ["--version"], { encoding: "utf8" }).status === 0) {
    return "linux-secret-service";
  }
  return null;
}

function nativeHelperPath() {
  if (process.platform !== "darwin") return null;
  const candidates = [
    process.env.SECONDEGO_CREDENTIAL_HELPER,
    app.isPackaged ? path.join(process.resourcesPath, "SecondEgoCredentialHelper") : null,
    path.join(__dirname, "..", "native", ".build", "SecondEgoCredentialHelper"),
  ].filter(Boolean);
  return candidates.find((candidate) => fs.existsSync(candidate)) || null;
}

function credentialId(provider, profile) {
  return `${assertProvider(provider)}:${assertSafeValue(profile, "Profile name")}`;
}

function readCredentials() {
  return readJson("credentials.json", {});
}

function readProfiles() {
  return readJson("profiles.json", { active: null, entries: {} });
}

function runNative(args, input = "") {
  const command = process.platform === "darwin" ? "security" : "secret-tool";
  const result = spawnSync(command, args, {
    input,
    encoding: "utf8",
    timeout: 10_000,
    maxBuffer: 64 * 1024,
  });
  if (result.error || result.status !== 0) throw new Error("The operating system credential vault rejected the request.");
  return result.stdout.trim();
}

function runMacHelper(args, input = "") {
  const helper = nativeHelperPath();
  if (!helper) throw new Error("The macOS credential helper is unavailable.");
  const result = spawnSync(helper, args, {
    input,
    encoding: "utf8",
    timeout: 10_000,
    maxBuffer: 64 * 1024,
  });
  if (result.error || result.status !== 0) throw new Error("The operating system credential vault rejected the request.");
  return result.stdout.trim();
}

function nativeCredential(provider, profile, action, secret = "") {
  const account = credentialId(provider, profile);
  if (nativeBackend() === "macos-keychain-helper") {
    return runMacHelper([action, KEYCHAIN_SERVICE, account], action === "save" ? `${secret}\n` : "");
  }
  if (nativeBackend() === "macos-keychain") {
    // A bare -w reads the password from stdin. Never put the secret in argv.
    if (action === "save") runNative(["add-generic-password", "-U", "-a", account, "-s", KEYCHAIN_SERVICE, "-w"], `${secret}\n`);
    if (action === "read") return runNative(["find-generic-password", "-a", account, "-s", KEYCHAIN_SERVICE, "-w"]);
    if (action === "delete") runNative(["delete-generic-password", "-a", account, "-s", KEYCHAIN_SERVICE]);
    return "";
  }
  if (nativeBackend() === "linux-secret-service") {
    if (action === "save") runNative(["store", "--label=SecondEgo provider key", "service", KEYCHAIN_SERVICE, "account", account], `${secret}\n`);
    if (action === "read") return runNative(["lookup", "service", KEYCHAIN_SERVICE, "account", account]);
    if (action === "delete") runNative(["clear", "service", KEYCHAIN_SERVICE, "account", account]);
    return "";
  }
  throw new Error("Native terminal profiles require macOS Keychain, Windows Credential Manager, or Linux Secret Service.");
}

function hasCredential(provider, profile) {
  try {
    if (nativeBackend()) {
      if (nativeCredential(provider, profile, "read")) return true;
    }
    const encoded = readCredentials()[credentialId(provider, profile)];
    return Boolean(encoded);
  } catch {
    return false;
  }
}

function readCredential(provider, profile) {
  if (nativeBackend()) {
    try { return nativeCredential(provider, profile, "read"); } catch { /* migrate legacy safeStorage below */ }
  }
  const encoded = readCredentials()[credentialId(provider, profile)];
  if (!encoded) throw new Error("Provider profile key is missing");
  ensureSecureStorage();
  return safeStorage.decryptString(Buffer.from(encoded, "base64"));
}

function saveProfile({ provider, profile, model, apiKey }) {
  provider = assertProvider(provider);
  profile = assertSafeValue(profile, "Profile name");
  if (typeof apiKey !== "string" || apiKey.trim().length < 8 || apiKey.length > 512) {
    throw new Error("API key must be between 8 and 512 characters");
  }
  ensureSecureStorage();
  const id = credentialId(provider, profile);
  if (nativeBackend()) {
    nativeCredential(provider, profile, "save", apiKey.trim());
  } else {
    const credentials = readCredentials();
    credentials[id] = safeStorage.encryptString(apiKey.trim()).toString("base64");
    writeJson("credentials.json", credentials);
  }

  const profiles = readProfiles();
  profiles.entries[id] = {
    provider,
    profile,
    model: typeof model === "string" && model.trim() ? model.trim().slice(0, 256) : null,
    updatedAt: new Date().toISOString(),
  };
  if (!profiles.active) profiles.active = id;
  writeJson("profiles.json", profiles);
  return { ...profiles.entries[id], id, keyPresent: true, envName: KEY_ENV[provider] };
}

function listProfiles() {
  const profiles = readProfiles();
  return {
    active: profiles.active,
    entries: Object.entries(profiles.entries || {}).map(([id, entry]) => ({
      ...entry,
      id,
      keyPresent: hasCredential(entry.provider, entry.profile),
      envName: KEY_ENV[entry.provider],
    })),
  };
}

function setActiveProfile({ provider, profile }) {
  const profiles = readProfiles();
  const id = credentialId(provider, profile);
  if (!profiles.entries[id] || !hasCredential(provider, profile)) throw new Error("That provider profile has no saved key");
  profiles.active = id;
  writeJson("profiles.json", profiles);
  return listProfiles();
}

function deleteProfile({ provider, profile }) {
  const id = credentialId(provider, profile);
  if (nativeBackend()) {
    try { nativeCredential(provider, profile, "delete"); } catch { /* already absent */ }
  } else {
    const credentials = readCredentials();
    delete credentials[id];
    writeJson("credentials.json", credentials);
  }
  const profiles = readProfiles();
  delete profiles.entries[id];
  if (profiles.active === id) profiles.active = Object.keys(profiles.entries)[0] || null;
  writeJson("profiles.json", profiles);
  return listProfiles();
}

function activeEnvironment() {
  const profiles = readProfiles();
  if (!profiles.active) return {};
  const entry = profiles.entries[profiles.active];
  if (!entry || !hasCredential(entry.provider, entry.profile)) return {};
  const key = readCredential(entry.provider, entry.profile);
  return {
    SECONDEGO_PROFILE_ID: profiles.active,
    SECONDEGO_PROVIDER: entry.provider,
    ...(entry.model ? { SECONDEGO_MODEL: entry.model } : {}),
    [KEY_ENV[entry.provider]]: key,
  };
}

module.exports = {
  PROVIDERS,
  activeEnvironment,
  deleteProfile,
  listProfiles,
  saveProfile,
  setActiveProfile,
};
