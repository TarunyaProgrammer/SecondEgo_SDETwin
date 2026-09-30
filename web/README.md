# SecondEgo marketing site

Static paper-and-ink marketing page built with Vite, React, TypeScript, Tailwind CSS, and Framer Motion. The page uses scroll-driven tone changes, a replayable proof console, interactive lifecycle tabs, and open-source project evidence.

## Run locally

Use Node.js 20.9 or newer.

```bash
npm install
npm run dev
```

The static bundle is written to `dist/` by `npm run build`. The page uses a local-first Avenir Next / DM Sans pairing with system fallbacks, so typography does not depend on a third-party font request.

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
