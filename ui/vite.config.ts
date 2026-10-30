import { defineConfig } from "vite";
import solid from "vite-plugin-solid";

export default defineConfig({
  plugins: [solid()],
  base: "./",
  // Flags stay separate files (loaded only when shown), never inlined into the scripts.
  build: { outDir: "dist", emptyOutDir: true, target: "es2022", chunkSizeWarningLimit: 900, assetsInlineLimit: (f: string) => (f.includes("flag-icons") ? false : undefined) },
});
