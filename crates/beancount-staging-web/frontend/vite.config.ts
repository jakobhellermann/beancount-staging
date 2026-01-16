/// <reference types="vitest/config" />
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
  },
  test: {
    environment: "happy-dom",
    globals: true,
    include: ["test/**/*.{test,spec}.ts"],
    coverage: {
      provider: "v8",
      reporter: ["text", "json", "html"],
      include: ["src/**/*.ts"],
      exclude: ["src/**/*.{test,spec}.ts"],
    },
  },
});
