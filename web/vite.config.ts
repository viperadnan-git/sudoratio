import tailwindcss from "@tailwindcss/vite";
import { tanstackRouter } from "@tanstack/router-plugin/vite";
import viteReact from "@vitejs/plugin-react";
import { defineConfig } from "vite";

const BACKEND = process.env.SUDORATIO_BACKEND ?? "http://127.0.0.1:8787";

// Pure SPA. The TanStack Router plugin must run before `@vitejs/plugin-react` so it can rewrite
// route files first. `vite build` produces a static `dist/` (index.html + content-addressed
// `assets/*`) that the Rust binary embeds via rust-embed.
export default defineConfig({
  resolve: { tsconfigPaths: true },
  plugins: [
    tanstackRouter({ target: "react", autoCodeSplitting: true }),
    viteReact(),
    tailwindcss(),
  ],
  server: {
    port: 3000,
    proxy: {
      // Forward API calls to the Rust backend during dev. In production the SPA is built
      // into `dist/` and served from the same origin by the Rust binary.
      "/api": {
        target: BACKEND,
        changeOrigin: true,
      },
    },
  },
});
