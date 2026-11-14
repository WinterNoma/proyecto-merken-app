import { defineConfig } from "vite";
// @ts-expect-error type error without @types/node package
import process from "node:process";
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(() => ({
  // La interfaz vive en ui/; Tauri lee el build desde dist/ en la raíz.
  root: "ui",
  build: {
    outDir: "../dist",
    emptyOutDir: true,
  },

  // Opciones para Tauri (`tauri dev` / `tauri build`)
  // 1. no ocultar los errores de Rust
  clearScreen: false,
  // 2. Tauri espera un puerto fijo
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
    watch: {
      // 3. no vigilar src-tauri ni crates (los recompila cargo)
      ignored: ["**/src-tauri/**", "**/crates/**"],
    },
  },
}));
