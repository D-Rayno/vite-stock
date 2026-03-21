import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "path";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react(), tailwindcss()],

  // ── Path alias ───────────────────────────────────────────────────────────
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },

  // ── Env prefix ───────────────────────────────────────────────────────────
  envPrefix: ["VITE_", "TAURI_"],

  // ── Vite / Tauri integration ─────────────────────────────────────────────
  // Prevent Vite from obscuring Rust compiler errors
  clearScreen: false,

  // ── Dev server ───────────────────────────────────────────────────────────
  server: {
    // Tauri expects a fixed port — fail hard if unavailable
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? { protocol: "ws", host, port: 1421 }
      : undefined,
    watch: {
      // Don't watch the Rust source — the Tauri CLI handles that side
      ignored: ["**/src-tauri/**"],
    },
  },

  // ── Production build ─────────────────────────────────────────────────────
  build: {
    // Tauri's embedded Webview2 on Windows maps to Chrome 105;
    // macOS WKWebView requires Safari 13+ compatibility.
    target:
      process.env.TAURI_PLATFORM === "windows" ? "chrome105" : "safari13",
    // Keep source readable in debug builds
    minify: process.env.TAURI_DEBUG ? false : "esbuild",
    sourcemap: !!process.env.TAURI_DEBUG,
  },
});