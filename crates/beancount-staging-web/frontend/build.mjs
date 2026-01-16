import { context, build } from "esbuild";
import { rmSync } from "fs";

const isWatch = process.argv.includes("--watch");

async function run() {
  try {
    rmSync("dist", { recursive: true });
  } catch (_) {}

  const buildConfig = {
    entryPoints: {
      app: "src/app.ts",
      index: "public/index.html",
      favicon: "public/favicon.svg",
    },
    bundle: true,
    outdir: "dist",
    sourcemap: true,
    target: "es2020",
    loader: {
      ".html": "copy",
      ".svg": "copy",
    },
  };

  if (isWatch) {
    // Watch mode
    const ctx = await context(buildConfig);
    await ctx.watch();
    console.log("👀 Watching for changes...");
  } else {
    // One-time build
    await build(buildConfig);
    console.log("✓ Build completed successfully");
  }
}

run().catch((error) => {
  console.error("Build failed:", error);
  process.exit(1);
});
