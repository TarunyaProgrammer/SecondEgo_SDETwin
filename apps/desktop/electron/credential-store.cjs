const { app, safeStorage } = require("electron");
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
  if (!safeStorage.isEncryptionAvailable()) {
    throw new Error("The operating system credential vault is not ready; restart SecondEgo and try again.");
  }
  if (typeof safeStorage.getSelectedStorageBackend === "function" && safeStorage.getSelectedStorageBackend() === "basic_text") {
    throw new Error("SecondEgo refused an unencrypted credential backend. Enable the OS keychain and retry.");
  }
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

function saveProfile({ provider, profile, model, apiKey }) {
  provider = assertProvider(provider);
  profile = assertSafeValue(profile, "Profile name");
  if (typeof apiKey !== "string" || apiKey.trim().length < 8 || apiKey.length > 512) {
    throw new Error("API key must be between 8 and 512 characters");
  }
  ensureSecureStorage();
  const id = credentialId(provider, profile);
  const credentials = readCredentials();
  credentials[id] = safeStorage.encryptString(apiKey.trim()).toString("base64");
  writeJson("credentials.json", credentials);

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
      keyPresent: Boolean(readCredentials()[id]),
      envName: KEY_ENV[entry.provider],
    })),
  };
}

function setActiveProfile({ provider, profile }) {
  const profiles = readProfiles();
  const id = credentialId(provider, profile);
  if (!profiles.entries[id] || !readCredentials()[id]) throw new Error("That provider profile has no saved key");
  profiles.active = id;
  writeJson("profiles.json", profiles);
  return listProfiles();
}

function deleteProfile({ provider, profile }) {
  const id = credentialId(provider, profile);
  const credentials = readCredentials();
  delete credentials[id];
  writeJson("credentials.json", credentials);
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
  const encoded = readCredentials()[profiles.active];
  if (!entry || !encoded) return {};
  ensureSecureStorage();
  const key = safeStorage.decryptString(Buffer.from(encoded, "base64"));
  return {
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
