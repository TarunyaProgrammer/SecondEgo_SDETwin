import { getAbsoluteAssetUrl, getCanonicalUrl, siteConfig } from "./site-config";

type PageMeta = {
  title: string;
  description: string;
};

const pageMeta: Record<string, PageMeta> = {
  "/": {
    title: "SecondEgo — The coding agent with a conscience",
    description: siteConfig.defaultDescription,
  },
  "/docs": {
    title: "SecondEgo Docs — Build with the whole picture",
    description: "Install SecondEgo, configure provider profiles, understand the local control plane, and debug a verified run.",
  },
  "/download": {
    title: "Download SecondEgo — Choose your platform",
    description: "Download the SecondEgo desktop app for macOS, Windows, or Linux and start with your own provider key.",
  },
  "/about": {
    title: "About SecondEgo — Software that knows its edges",
    description: "Learn why SecondEgo is built around local execution, explicit boundaries, and evidence-backed software changes.",
  },
  "/troubleshooting": {
    title: "Troubleshooting SecondEgo — Recover with evidence",
    description: "Resolve gateway, provider, download, and diff issues with bounded recovery steps for SecondEgo.",
  },
  "/terms": {
    title: "Terms and Conditions — SecondEgo",
    description: "Read the product-facing terms and responsibilities for using SecondEgo.",
  },
};

function setMeta(attribute: "name" | "property", key: string, value: string) {
  let element = document.head.querySelector<HTMLMetaElement>(`meta[${attribute}="${key}"]`);
  if (!element) {
    element = document.createElement("meta");
    element.setAttribute(attribute, key);
    document.head.appendChild(element);
  }
  element.content = value;
}

function setCanonical(path: string) {
  let link = document.head.querySelector<HTMLLinkElement>('link[rel="canonical"]');
  if (!link) {
    link = document.createElement("link");
    link.rel = "canonical";
    document.head.appendChild(link);
  }
  link.href = getCanonicalUrl(path);
}

function setStructuredData(path: string, title: string, description: string) {
  const existing = document.getElementById("secondego-jsonld");
  const element = existing instanceof HTMLScriptElement ? existing : document.createElement("script");
  element.id = "secondego-jsonld";
  element.type = "application/ld+json";
  element.textContent = JSON.stringify({
    "@context": "https://schema.org",
    "@type": "WebPage",
    name: title,
    description,
    url: getCanonicalUrl(path),
    isPartOf: { "@type": "WebSite", name: siteConfig.siteName, url: siteConfig.siteUrl },
    publisher: { "@type": "Organization", name: siteConfig.organization.name, url: siteConfig.organization.url },
  });
  if (!element.parentElement) document.head.appendChild(element);
}

export function applyPageMeta(path: string) {
  const normalized = path === "/downloads" ? "/download" : path === "/terms-and-conditions" ? "/terms" : path;
  const meta = pageMeta[normalized] || pageMeta["/"];
  const image = getAbsoluteAssetUrl(siteConfig.defaultOgImage);
  document.title = meta.title;
  setMeta("name", "description", meta.description);
  setMeta("property", "og:title", meta.title);
  setMeta("property", "og:description", meta.description);
  setMeta("property", "og:url", getCanonicalUrl(normalized));
  setMeta("property", "og:image", image);
  setMeta("name", "twitter:title", meta.title);
  setMeta("name", "twitter:description", meta.description);
  setMeta("name", "twitter:image", image);
  setMeta("name", "twitter:url", getCanonicalUrl(normalized));
  setCanonical(normalized);
  setStructuredData(normalized, meta.title, meta.description);
}
