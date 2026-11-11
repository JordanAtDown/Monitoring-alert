import { defineConfig } from "vite";

// The Rust executable embeds a fixed list of files (see backend/build.rs),
// so the output names are stable instead of content-hashed.
export default defineConfig({
  build: {
    outDir: "dist",
    cssCodeSplit: false,
    rollupOptions: {
      output: {
        entryFileNames: "assets/app.js",
        assetFileNames: (asset) =>
          asset.names.some((n) => n.endsWith(".css")) ? "assets/app.css" : "assets/[name][extname]",
      },
    },
  },
  server: {
    // `npm run dev` while `monitoring-alert serve` runs on its default port.
    proxy: { "/api": "http://127.0.0.1:8787" },
  },
});
