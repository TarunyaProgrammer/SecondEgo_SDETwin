const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");

const releaseDirectory = path.join(__dirname, "..", "release");
const packageJson = JSON.parse(fs.readFileSync(path.join(__dirname, "..", "package.json"), "utf8"));
const version = process.env.RELEASE_VERSION || process.env.GITHUB_REF_NAME || packageJson.version;
const signed = process.env.RELEASE_SIGNED === "true";
const notarized = process.env.RELEASE_NOTARIZED === "true";
const artifacts = fs.readdirSync(releaseDirectory)
  .filter((name) => name.endsWith(".dmg"))
  .sort()
  .map((name) => {
    const file = path.join(releaseDirectory, name);
    const sha256 = crypto.createHash("sha256").update(fs.readFileSync(file)).digest("hex");
    const architecture = name.includes("arm64") ? "arm64" : name.includes("x64") ? "x64" : "unknown";
    return { name, architecture, sha256, bytes: fs.statSync(file).size };
  });

if (!artifacts.length) throw new Error(`No DMG artifacts found in ${releaseDirectory}`);

const manifest = {
  schema_version: 1,
  product: "SecondEgo",
  version,
  platform: "macOS",
  signed,
  notarized,
  update_channel: signed ? "manual-github-release-signed" : "manual-github-release-candidate",
  artifacts,
};

fs.writeFileSync(path.join(releaseDirectory, "release-manifest.json"), `${JSON.stringify(manifest, null, 2)}\n`);
fs.writeFileSync(
  path.join(releaseDirectory, "SHA256SUMS.txt"),
  `${artifacts.map((artifact) => `${artifact.sha256}  ${artifact.name}`).join("\n")}\n`,
);
console.log(`Wrote release-manifest.json and SHA256SUMS.txt for ${artifacts.length} artifact(s).`);
