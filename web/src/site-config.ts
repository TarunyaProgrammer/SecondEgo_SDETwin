/**
 * Canonical Site Configuration for SecondEgo
 * Single source of truth for SEO, structured data, canonical URLs, and social sharing.
 *
 * NOTE: Set `VITE_SITE_URL` or `SITE_URL` in your deployment environment
 * (e.g. Vercel, Netlify, Cloudflare Pages, or GitHub Pages) to match your custom production domain.
 * If not set, it defaults to the canonical placeholder "https://YOUR_PRODUCTION_DOMAIN".
 */

// Isolated configuration variable for canonical production origin
export const SITE_URL = (
  (typeof process !== "undefined" && process.env?.SITE_URL) ||
  (typeof process !== "undefined" && process.env?.VITE_SITE_URL) ||
  (typeof import.meta !== "undefined" && import.meta.env?.VITE_SITE_URL) ||
  "https://YOUR_PRODUCTION_DOMAIN"
).replace(/\/+$/, "");

export const siteConfig = {
  productName: "SecondEgo",
  siteName: "SecondEgo",
  tagline: "Verified change, built in the open",
  defaultTitle: "SecondEgo — Verified Code Changes You Can Defend | Local AI Coding Harness",
  titleTemplate: "%s | SecondEgo",
  defaultDescription:
    "SecondEgo is an open-source, local-first AI coding-agent harness that turns software issues into isolated, verified diffs with bounded model planning, transactional worktree isolation, and deterministic test evidence.",
  keywords: [
    "AI coding agent",
    "coding agent harness",
    "local coding harness",
    "software engineering agent",
    "Rust coding agent",
    "repository intelligence",
    "transactional worktree",
    "verified diff",
    "bounded execution",
    "open source AI harness",
  ],
  siteUrl: SITE_URL,
  defaultOgImage: "/og-image.png",
  defaultTwitterImage: "/twitter-image.png",
  logo: "/potential-logo.png",
  logoSquare: "/favicon.png",
  logoSvg: "/icon.svg",
  favicon: "/favicon.ico",
  appleTouchIcon: "/apple-touch-icon.png",
  locale: "en_US",
  themeColor: "#f3f2ee",
  themeColorDark: "#10100f",
  twitterCardType: "summary_large_image" as const,
  repository: "https://github.com/TarunyaProgrammer/SecondEgo_SDETwin",
  license: "https://github.com/TarunyaProgrammer/SecondEgo_SDETwin/blob/main/LICENSE",
  security: "https://github.com/TarunyaProgrammer/SecondEgo_SDETwin/blob/main/SECURITY.md",
  organization: {
    name: "SecondEgo",
    url: SITE_URL,
    logo: `${SITE_URL}/potential-logo.png`,
    sameAs: [
      "https://github.com/TarunyaProgrammer/SecondEgo_SDETwin",
    ],
  },
  team: [
    {
      name: "Tarunya Kesharwani",
      role: "Runtime Architect & Systems Lead",
      github: "https://github.com/TarunyaProgrammer",
      linkedin: "https://www.linkedin.com/in/tarunyakesharwani",
    },
    {
      name: "Riya Agarwal",
      role: "Full-Stack & Systems Engineer",
      github: "https://github.com/riyaagarwal5040",
      linkedin: "https://www.linkedin.com/in/riya5040agarwal",
    },
    {
      name: "Mansha Agarwal",
      role: "Frontend Engineer & Code Owner",
      github: "https://github.com/ManshaAgarwal716",
      linkedin: "https://www.linkedin.com/in/mansha-agarwal-198759369",
    },
    {
      name: "Yashika Gupta",
      role: "Frontend & Web Experience Engineer",
      github: "https://github.com/YashikaGupta2407",
      linkedin: "https://www.linkedin.com/in/yashika-gupta-08781a2b7",
    },
  ],
} as const;

/**
 * Returns an absolute canonical URL for a given path.
 */
export function getCanonicalUrl(path = "/"): string {
  const cleanPath = path.startsWith("/") ? path : `/${path}`;
  return `${SITE_URL}${cleanPath === "/" ? "" : cleanPath}`;
}

/**
 * Returns an absolute URL for an asset path.
 */
export function getAbsoluteAssetUrl(assetPath: string): string {
  if (assetPath.startsWith("http://") || assetPath.startsWith("https://")) {
    return assetPath;
  }
  const cleanPath = assetPath.startsWith("/") ? assetPath : `/${assetPath}`;
  return `${SITE_URL}${cleanPath}`;
}
