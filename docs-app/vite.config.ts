import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// base "./" keeps the bundle path-independent so it can be served from
// desktop/public/docs/ (Tauri and ira-server) or from its own dev port.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  base: "./",
  build: {
    outDir: "../desktop/public/docs",
    emptyOutDir: true,
  },
  server: {
    port: 5190,
    strictPort: true,
    host: "127.0.0.1",
  },
});
