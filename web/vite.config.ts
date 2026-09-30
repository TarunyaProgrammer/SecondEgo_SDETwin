import { defineConfig, loadEnv } from "vite";
import react from "@vitejs/plugin-react";
import { fileURLToPath, URL } from "node:url";

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), "");
  const siteUrl = (process.env.SITE_URL || env.SITE_URL || env.VITE_SITE_URL || "https://YOUR_PRODUCTION_DOMAIN").replace(/\/+$/, "");

  return {
    plugins: [
      react(),
      {
        name: "seo-html-transform",
        transformIndexHtml(html: string) {
          if (siteUrl && siteUrl !== "https://YOUR_PRODUCTION_DOMAIN") {
            return html.replaceAll("https://YOUR_PRODUCTION_DOMAIN", siteUrl);
          }
          return html;
        },
      },
    ],
    resolve: {
      alias: {
        "@": fileURLToPath(new URL(".", import.meta.url)),
      },
    },
    build: { target: "es2020" },
  };
});
