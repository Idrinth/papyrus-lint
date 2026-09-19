import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import { injectContactLinks } from "./scripts/inject-links.mjs";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;
const appDir = path.dirname(fileURLToPath(import.meta.url));
const sharedDir = path.resolve(appDir, "../shared");

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [injectContactLinks()],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    fs: {
      // shared/theme.css lives one directory above the Vite root
      allow: [appDir, sharedDir],
    },
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
