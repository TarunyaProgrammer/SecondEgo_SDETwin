# SecondEgo product site

Static product and documentation site built with Vite, React, and TypeScript. It ships as a dark, red-black product surface with route-aware Home, Docs, Download, About, Troubleshooting, and Terms pages.

## Run locally

Use Node.js 20.9 or newer.

```bash
npm install
npm run dev
```

The static bundle is written to `dist/` by `npm run build`. The site uses Manrope, DM Sans, and Cormorant Garamond with system fallbacks; there is no terminal-style font in the product UI. If a deployment must be fully offline, remove the Google Fonts import in `src/styles.css` and provide local font assets.

Routes are handled by the lightweight client router in `src/App.tsx`: `/`, `/docs`, `/download`, `/about`, `/troubleshooting`, `/terms`, and `/terms-and-conditions`. The download page detects macOS, Windows, or Linux from the browser user agent and highlights the matching release card. The website is the only public download gateway; a platform is enabled only when its exact signed artifact URL is configured at build time.

Published deployments can promote exact artifact links without changing the UI:

```bash
VITE_DOWNLOAD_MAC_URL="https://.../SecondEgo-arm64.dmg" \\
VITE_DOWNLOAD_WINDOWS_URL="https://.../SecondEgo-x64.exe" \\
VITE_DOWNLOAD_LINUX_URL="https://.../SecondEgo-x64.AppImage" \\
npm run build
```

When a URL is absent, the platform card stays unavailable and does not link to
GitHub or any secondary download surface. This keeps the website deployable
before the first signed artifacts exist without violating the distribution
policy.

The included `public/_redirects` and `vercel.json` keep those history routes working on common static hosts. Configure an equivalent SPA fallback if deploying elsewhere.

## Production deployment & canonical domain

Configure the canonical production URL by setting `SITE_URL` (or `VITE_SITE_URL`) in your build environment:

```bash
SITE_URL="https://secondego.org" npm run build
```

If `SITE_URL` is omitted, the build defaults to the placeholder `https://YOUR_PRODUCTION_DOMAIN/` as specified in [`src/site-config.ts`](./src/site-config.ts).

To regenerate the 1200×630 Open Graph preview image asset:

```bash
npm run generate:og
```

The generator uses the portable `@resvg/resvg-js` renderer, so it does not
depend on macOS `sips` or an ImageMagick installation in CI.

The architecture copy is based on the current project context and evaluation materials. The recovery panel is an architecture walkthrough, not a customer case study. The footer signup field is intentionally a status message; it does not submit email addresses anywhere. Release jobs produce platform-specific artifacts, checksums, and manifests; signing, notarization, direct website URLs, and update channels remain release-infrastructure gates.
