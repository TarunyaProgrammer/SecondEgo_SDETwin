import fs from "node:fs";
import { execSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const webDir = path.resolve(__dirname, "..");
const logoPath = path.resolve(webDir, "public/potential-logo.png");

// Read Potential Logo PNG and convert to base64
const logoBase64 = fs.readFileSync(logoPath).toString("base64");
const logoDataUri = `data:image/png;base64,${logoBase64}`;

const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="630" viewBox="0 0 1200 630">
  <defs>
    <radialGradient id="coralGlow" cx="20%" cy="30%" r="60%">
      <stop offset="0%" stop-color="#ED4520" stop-opacity="0.18"/>
      <stop offset="60%" stop-color="#ED4520" stop-opacity="0.03"/>
      <stop offset="100%" stop-color="#0c0e12" stop-opacity="0"/>
    </radialGradient>
    <radialGradient id="meshGlow" cx="80%" cy="70%" r="50%">
      <stop offset="0%" stop-color="#3b82f6" stop-opacity="0.08"/>
      <stop offset="100%" stop-color="#0c0e12" stop-opacity="0"/>
    </radialGradient>
    <pattern id="grid" width="40" height="40" patternUnits="userSpaceOnUse">
      <path d="M 40 0 L 0 0 0 40" fill="none" stroke="rgba(255,255,255,0.03)" stroke-width="1"/>
    </pattern>
    <filter id="cardShadow" x="-10%" y="-10%" width="120%" height="120%">
      <feDropShadow dx="0" dy="16" stdDeviation="24" flood-color="#000000" flood-opacity="0.5"/>
    </filter>
  </defs>

  <!-- Background -->
  <rect width="1200" height="630" fill="#0C0E12"/>
  <rect width="1200" height="630" fill="url(#grid)"/>
  <rect width="1200" height="630" fill="url(#coralGlow)"/>
  <rect width="1200" height="630" fill="url(#meshGlow)"/>

  <!-- Outer frame border -->
  <rect x="24" y="24" width="1152" height="582" rx="16" fill="none" stroke="rgba(255,255,255,0.08)" stroke-width="1"/>

  <!-- Top Bar: Brand & Product category -->
  <g transform="translate(64, 60)">
    <!-- Logo image badge -->
    <image href="${logoDataUri}" x="0" y="0" width="72" height="40" preserveAspectRatio="xMidYMid meet"/>
    <!-- Brand wordmark -->
    <text x="86" y="28" font-family="-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif" font-size="28" font-weight="700" fill="#F4F1EA" letter-spacing="-0.04em">second<tspan fill="#8E8D86" font-weight="400">ego</tspan></text>
    <!-- Category pill -->
    <rect x="245" y="7" width="220" height="26" rx="13" fill="rgba(237, 69, 32, 0.12)" stroke="rgba(237, 69, 32, 0.3)" stroke-width="1"/>
    <text x="355" y="24" font-family="'SF Mono', Menlo, Consolas, monospace" font-size="11" font-weight="600" fill="#ED4520" text-anchor="middle" letter-spacing="0.08em">LOCAL AI CODING HARNESS</text>
  </g>

  <!-- Left Main Content: Headline and Subhead -->
  <g transform="translate(64, 155)">
    <!-- Section eyebrow -->
    <text x="0" y="16" font-family="'SF Mono', Menlo, Consolas, monospace" font-size="12" font-weight="600" fill="#8B8A82" letter-spacing="0.14em">BOUNDED EXECUTION · VERIFIED DIFFS</text>

    <!-- Main Headline -->
    <text x="0" y="85" font-family="-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif" font-size="62" font-weight="800" fill="#F5F3EC" letter-spacing="-0.04em">
      Make code changes
    </text>
    <text x="0" y="155" font-family="-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif" font-size="62" font-weight="800" fill="#ED4520" letter-spacing="-0.04em">
      you can defend.
    </text>

    <!-- Subhead description -->
    <text x="0" y="215" font-family="-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif" font-size="20" font-weight="400" fill="#A7A59C">SecondEgo turns a software issue into an</text>
    <text x="0" y="244" font-family="-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif" font-size="20" font-weight="400" fill="#A7A59C">isolated, verified diff. The model can propose.</text>
    <text x="0" y="273" font-family="-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif" font-size="20" font-weight="400" fill="#A7A59C">The runtime still has to prove it.</text>

    <!-- Feature badges -->
    <g transform="translate(0, 318)">
      <!-- Badge 1 -->
      <rect x="0" y="0" width="130" height="34" rx="8" fill="rgba(255,255,255,0.04)" stroke="rgba(255,255,255,0.12)" stroke-width="1"/>
      <circle cx="16" cy="17" r="4" fill="#22c55e"/>
      <text x="28" y="22" font-family="'SF Mono', Menlo, monospace" font-size="12" font-weight="500" fill="#E2E0D8">Local-First</text>

      <!-- Badge 2 -->
      <rect x="142" y="0" width="136" height="34" rx="8" fill="rgba(255,255,255,0.04)" stroke="rgba(255,255,255,0.12)" stroke-width="1"/>
      <circle cx="158" cy="17" r="4" fill="#ED4520"/>
      <text x="170" y="22" font-family="'SF Mono', Menlo, monospace" font-size="12" font-weight="500" fill="#E2E0D8">Rust Engine</text>

      <!-- Badge 3 -->
      <rect x="290" y="0" width="144" height="34" rx="8" fill="rgba(255,255,255,0.04)" stroke="rgba(255,255,255,0.12)" stroke-width="1"/>
      <circle cx="306" cy="17" r="4" fill="#3b82f6"/>
      <text x="318" y="22" font-family="'SF Mono', Menlo, monospace" font-size="12" font-weight="500" fill="#E2E0D8">MIT Licensed</text>

      <!-- Badge 4 -->
      <rect x="446" y="0" width="132" height="34" rx="8" fill="rgba(255,255,255,0.04)" stroke="rgba(255,255,255,0.12)" stroke-width="1"/>
      <circle cx="462" cy="17" r="4" fill="#a855f7"/>
      <text x="474" y="22" font-family="'SF Mono', Menlo, monospace" font-size="12" font-weight="500" fill="#E2E0D8">Zero Cloud</text>
    </g>
  </g>

  <!-- Right Visual: The Architectural Boundary Panel -->
  <g transform="translate(680, 140)" filter="url(#cardShadow)">
    <!-- Panel container -->
    <rect width="456" height="375" rx="14" fill="#13161C" stroke="rgba(255,255,255,0.1)" stroke-width="1"/>
    
    <!-- Window header -->
    <rect width="456" height="42" rx="14" fill="#181B22"/>
    <rect y="32" width="456" height="10" fill="#181B22"/>
    <line x1="0" y1="42" x2="456" y2="42" stroke="rgba(255,255,255,0.07)" stroke-width="1"/>
    
    <!-- Window dots -->
    <circle cx="22" cy="21" r="5" fill="#EF4444" opacity="0.8"/>
    <circle cx="38" cy="21" r="5" fill="#F59E0B" opacity="0.8"/>
    <circle cx="54" cy="21" r="5" fill="#10B981" opacity="0.8"/>
    <text x="74" y="25" font-family="'SF Mono', Menlo, monospace" font-size="11" font-weight="500" fill="#71717A">secondego · run-boundary</text>
    <rect x="330" y="11" width="108" height="20" rx="4" fill="rgba(34,197,94,0.12)"/>
    <text x="384" y="24" font-family="'SF Mono', Menlo, monospace" font-size="10" font-weight="600" fill="#22C55E" text-anchor="middle">VERIFIED PASS</text>

    <!-- Rail and Stages -->
    <g transform="translate(24, 60)">
      <!-- Vertical connecting line -->
      <line x1="16" y1="20" x2="16" y2="200" stroke="rgba(255,255,255,0.12)" stroke-width="2"/>
      <line x1="16" y1="20" x2="16" y2="140" stroke="#ED4520" stroke-width="2"/>

      <!-- Stage 1 -->
      <circle cx="16" cy="20" r="6" fill="#13161C" stroke="#38BDF8" stroke-width="3"/>
      <text x="36" y="18" font-family="'SF Mono', Menlo, monospace" font-size="12" font-weight="700" fill="#38BDF8">01 MODEL</text>
      <text x="120" y="18" font-family="'SF Mono', Menlo, monospace" font-size="11" fill="#71717A">/ structured AST plan</text>
      <text x="36" y="34" font-family="-apple-system, sans-serif" font-size="11" fill="#A1A1AA">Deterministic tool invocation</text>

      <!-- Stage 2 -->
      <circle cx="16" cy="75" r="6" fill="#13161C" stroke="#A78BFA" stroke-width="3"/>
      <text x="36" y="73" font-family="'SF Mono', Menlo, monospace" font-size="12" font-weight="700" fill="#A78BFA">02 RUNTIME</text>
      <text x="134" y="73" font-family="'SF Mono', Menlo, monospace" font-size="11" fill="#71717A">/ validates actions</text>
      <text x="36" y="89" font-family="-apple-system, sans-serif" font-size="11" fill="#A1A1AA">Pre-commit policy gates</text>

      <!-- Stage 3 -->
      <circle cx="16" cy="130" r="6" fill="#13161C" stroke="#34D399" stroke-width="3"/>
      <text x="36" y="128" font-family="'SF Mono', Menlo, monospace" font-size="12" font-weight="700" fill="#34D399">03 WORKTREE</text>
      <text x="144" y="128" font-family="'SF Mono', Menlo, monospace" font-size="11" fill="#71717A">/ isolated git diff</text>
      <text x="36" y="144" font-family="-apple-system, sans-serif" font-size="11" fill="#A1A1AA">Transactional branch sandbox</text>

      <!-- Stage 4 -->
      <circle cx="16" cy="185" r="6" fill="#13161C" stroke="#FBBF24" stroke-width="3"/>
      <text x="36" y="183" font-family="'SF Mono', Menlo, monospace" font-size="12" font-weight="700" fill="#FBBF24">04 VERIFY</text>
      <text x="122" y="183" font-family="'SF Mono', Menlo, monospace" font-size="11" fill="#71717A">/ evidence decides completion</text>
      <text x="36" y="199" font-family="-apple-system, sans-serif" font-size="11" fill="#A1A1AA">Tests, lints &amp; contracts pass</text>
    </g>

    <!-- Bottom summary box -->
    <rect x="20" y="295" width="416" height="58" rx="8" fill="#1A1E26" stroke="rgba(255,255,255,0.06)" stroke-width="1"/>
    <text x="36" y="322" font-family="'SF Mono', Menlo, monospace" font-size="11" font-weight="600" fill="#71717A">DECISION OWNERSHIP</text>
    <text x="36" y="340" font-family="'SF Mono', Menlo, monospace" font-size="13" font-weight="700" fill="#ED4520">PROVEN BY RUNTIME EVIDENCE</text>
    <rect x="330" y="309" width="92" height="30" rx="6" fill="#ED4520"/>
    <text x="376" y="329" font-family="-apple-system, sans-serif" font-size="12" font-weight="700" fill="#FFFFFF" text-anchor="middle">DIFF READY</text>
  </g>

  <!-- Bottom Bar: GitHub Source of Truth -->
  <g transform="translate(64, 555)">
    <text x="0" y="16" font-family="'SF Mono', Menlo, Consolas, monospace" font-size="12" fill="#6B6963">
      OPEN SOURCE ON GITHUB: <tspan fill="#ED4520" font-weight="600">github.com/TarunyaProgrammer/SecondEgo_SDETwin</tspan>
    </text>
    <text x="1072" y="16" font-family="'SF Mono', Menlo, Consolas, monospace" font-size="12" fill="#6B6963" text-anchor="end">
      RUST 2024 · GEMINI · PYTHON COMPAT
    </text>
  </g>
</svg>`;

const tmpSvg = path.resolve(webDir, "public/og-image.svg");
const outPng = path.resolve(webDir, "public/og-image.png");
const twitterPng = path.resolve(webDir, "public/twitter-image.png");

fs.writeFileSync(tmpSvg, svg.trim());
execSync(`sips -s format png "${tmpSvg}" --out "${outPng}"`);
fs.copyFileSync(outPng, twitterPng);

console.log("Generated og-image.png and twitter-image.png successfully!");
