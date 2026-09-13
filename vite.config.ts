import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// The page lives in frontend/; Tauri points at frontend/dist. Port 1420 is
// what crates/brokey/tauri.conf.json's devUrl names, and strictPort makes a
// clash an error rather than a silently different page.
export default defineConfig({
  root: "frontend",
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: { outDir: "dist", emptyOutDir: true, target: "safari15" },
  envPrefix: ["VITE_", "TAURI_"],
});
