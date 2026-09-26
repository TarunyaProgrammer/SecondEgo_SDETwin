import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  // The macOS shell loads the production bundle from file://, so asset URLs
  // must be relative to dist/ rather than rooted at the filesystem.
  base: "./",
  server: {
    host: "127.0.0.1",
    port: 5173,
    strictPort: true,
  },
});
