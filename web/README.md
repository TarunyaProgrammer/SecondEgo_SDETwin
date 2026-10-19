# SecondEgo product site

Static product and documentation site built with Vite, React, TypeScript, and Framer Motion. The page is organized around activation: provider profiles, terminal/desktop installation paths, HLD/LLD architecture, security boundaries, and links to the source documentation.

## Run locally

Use Node.js 20.9 or newer.

```bash
npm install
npm run dev
```

The static bundle is written to `dist/` by `npm run build`. The site uses Bricolage Grotesque, IBM Plex Sans, and DM Mono with system fallbacks. If a deployment must be fully offline, remove the Google Fonts import in `src/styles.css` and provide local font assets.

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

The architecture copy is based on the current project context and evaluation materials. The recovery panel is an architecture walkthrough, not a customer case study. The footer signup field is intentionally a status message; it does not submit email addresses anywhere.
