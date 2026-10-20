const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");

const releaseDirectory = path.join(__dirname, "..", "release");
const packageJson = JSON.parse(fs.readFileSync(path.join(__dirname, "..", "package.json"), "utf8"));
const version = process.env.RELEASE_VERSION || process.env.GITHUB_REF_NAME || packageJson.version;
const signed = process.env.RELEASE_SIGNED === "true";
const notarized = process.env.RELEASE_NOTARIZED === "true";

function platformForArtifact(name) {
  if (/\.dmg$/i.test(name)) return "macOS";
  if (/\.(exe|msi|nsis\.zip)$/i.test(name)) return "Windows";
  if (/\.(AppImage|deb|rpm|snap)$/i.test(name)) return "Linux";
  return "unknown";
}

function architectureForArtifact(name) {
  if (/arm64|aarch64/i.test(name)) return "arm64";
  if (/x64|x86_64|amd64/i.test(name)) return "x64";
  if (/ia32|x86/i.test(name)) return "ia32";
  return "unknown";
}

const artifacts = fs.readdirSync(releaseDirectory)
  .filter((name) => /\.(dmg|exe|msi|AppImage|deb|rpm|snap)$/i.test(name))
  .sort()
  .map((name) => {
    const file = path.join(releaseDirectory, name);
    return {
      name,
      platform: platformForArtifact(name),
      architecture: architectureForArtifact(name),
      sha256: crypto.createHash("sha256").update(fs.readFileSync(file)).digest("hex"),
      bytes: fs.statSync(file).size,
    };
  });

if (!artifacts.length) throw new Error(`No supported release artifacts found in ${releaseDirectory}`);

const platform = process.env.RELEASE_PLATFORM || [...new Set(artifacts.map((artifact) => artifact.platform))][0] || "unknown";
const slug = platform.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "") || "unknown";
const architecture = process.env.RELEASE_ARCH || [...new Set(artifacts.map((artifact) => artifact.architecture))][0] || "unknown";
const architectureSlug = architecture.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "") || "unknown";
const artifactStem = `${slug}-${architectureSlug}`;
const manifestName = `release-manifest-${artifactStem}.json`;
const checksumName = `SHA256SUMS-${artifactStem}.txt`;
const manifest = {
  schema_version: 2,
  product: "SecondEgo",
  version,
  platform,
  architecture,
  signed,
  notarized,
  update_channel: signed ? `manual-github-release-${slug}-signed` : `manual-github-release-${slug}-candidate`,
  artifacts,
};

fs.writeFileSync(path.join(releaseDirectory, manifestName), `${JSON.stringify(manifest, null, 2)}\n`);
fs.writeFileSync(
  path.join(releaseDirectory, checksumName),
  `${artifacts.map((artifact) => `${artifact.sha256}  ${artifact.name}`).join("\n")}\n`,
);
console.log(`Wrote ${manifestName} and ${checksumName} for ${artifacts.length} artifact(s).`);
