import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Development only: `PERO_DEV_SERVER=https://host:8443 npm run dev` forwards the API and health calls
// to a real Pero server (its certificate is not checked here, so use it for a local test server only).
const target = process.env["PERO_DEV_SERVER"];
const proxy = target
  ? Object.fromEntries(
      ["/api", "/health"].map((path) => [path, { target, secure: false, changeOrigin: true }]),
    )
  : undefined;

export default defineConfig({
  plugins: [svelte()],
  server: { proxy },
  preview: { proxy },
  build: { outDir: "dist", emptyOutDir: true, sourcemap: false },
});
