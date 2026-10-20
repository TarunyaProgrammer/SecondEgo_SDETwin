const fs = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const desktopRoot = path.join(__dirname, "..");
const repositoryRoot = path.join(desktopRoot, "..", "..");
const gatewayBinaryName = process.platform === "win32" ? "secondego-gateway.exe" : "secondego-gateway";
const source = process.env.SECONDEGO_GATEWAY_SOURCE
  ? path.resolve(process.env.SECONDEGO_GATEWAY_SOURCE)
  : path.join(repositoryRoot, "engine-rs", "target", "release", gatewayBinaryName);
const targetDirectory = path.join(desktopRoot, "resources");
const target = path.join(targetDirectory, gatewayBinaryName);

if (!fs.existsSync(source)) {
  throw new Error(`Missing release gateway: ${source}. Run cargo build --release -p secondego-gateway first.`);
}
fs.mkdirSync(targetDirectory, { recursive: true });
fs.copyFileSync(source, target);
fs.chmodSync(target, 0o755);
console.log(`Staged Rust gateway at ${target}`);

if (process.platform === "darwin") {
  const helperSource = path.join(desktopRoot, "native", "CredentialHelper.swift");
  const helperTarget = path.join(targetDirectory, "SecondEgoCredentialHelper");
  const moduleCache = path.join(desktopRoot, "native", ".build", "module-cache");
  fs.mkdirSync(moduleCache, { recursive: true });
  const result = spawnSync("swiftc", ["-target", `${process.arch === "arm64" ? "arm64" : "x86_64"}-apple-macosx11.0`, "-framework", "Foundation", "-framework", "Security", "-O", "-o", helperTarget, helperSource], {
    encoding: "utf8",
    env: { ...process.env, CLANG_MODULE_CACHE_PATH: moduleCache },
  });
  if (result.status !== 0) {
    throw new Error(`Could not build the macOS credential helper: ${result.stderr || result.error?.message || "swiftc failed"}`);
  }
  fs.chmodSync(helperTarget, 0o755);
  console.log(`Staged macOS credential helper at ${helperTarget}`);
}
