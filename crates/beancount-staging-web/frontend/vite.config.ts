import { defineConfig } from "vite";

export default defineConfig({
  publicDir: "public",
  base: "./",
  server: {
    proxy: {
      "/api": {
        target: "http://localhost:8472",
        changeOrigin: true,
      },
    },
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
    rollupOptions: {
      input: {
        main: "./index.html",
      },
    },
  },
});
