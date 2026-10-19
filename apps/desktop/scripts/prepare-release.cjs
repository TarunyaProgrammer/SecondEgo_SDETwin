const fs = require("node:fs");
const path = require("node:path");

const desktopRoot = path.join(__dirname, "..");
const repositoryRoot = path.join(desktopRoot, "..", "..");
const source = path.join(repositoryRoot, "engine-rs", "target", "release", "secondego-gateway");
const targetDirectory = path.join(desktopRoot, "resources");
const target = path.join(targetDirectory, "secondego-gateway");

if (!fs.existsSync(source)) {
  throw new Error(`Missing release gateway: ${source}. Run cargo build --release -p secondego-gateway first.`);
}
fs.mkdirSync(targetDirectory, { recursive: true });
fs.copyFileSync(source, target);
fs.chmodSync(target, 0o755);
console.log(`Staged Rust gateway at ${target}`);
