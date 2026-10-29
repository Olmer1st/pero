import { readFileSync } from "node:fs";
import { defineConfig, type Plugin } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Development only: `PERO_DEV_SERVER=https://host:8443 npm run dev` forwards the API and health calls
// to a real Pero server (its certificate is not checked here, so use it for a local test server only).
const target = process.env["PERO_DEV_SERVER"];
const proxy = target
  ? Object.fromEntries(
      ["/api", "/health"].map((path) => [path, { target, secure: false, changeOrigin: true }]),
    )
  : undefined;

// Preview only: behaves like the real server for the page (core/src/web.rs): a nonce for the editor's
// styles is written into index.html and into the Content-Security-Policy, so `npm run build &&
// npm run preview` shows whether CodeMirror works under the strict policy.
const previewCsp: Plugin = {
  name: "pero-preview-csp",
  configurePreviewServer(server) {
    server.middlewares.use((req, res, next) => {
      if (req.url !== "/" && req.url !== "/index.html") return next();
      const nonce = Math.random().toString(16).slice(2, 18).padEnd(16, "0");
      const html = readFileSync("dist/index.html", "utf8").replace("__CSP_NONCE__", nonce);
      res.setHeader("Content-Type", "text/html; charset=utf-8");
      res.setHeader(
        "Content-Security-Policy",
        `default-src 'none'; script-src 'self'; style-src 'self' 'nonce-${nonce}'; img-src 'self' data:; ` +
          "font-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'",
      );
      res.end(html);
    });
  },
};

export default defineConfig({
  plugins: [svelte(), previewCsp],
  server: { proxy },
  preview: { proxy },
  build: { outDir: "dist", emptyOutDir: true, sourcemap: false, chunkSizeWarningLimit: 700 },
});
