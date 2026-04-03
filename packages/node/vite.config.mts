import { builtinModules } from "node:module";
import { resolve } from "node:path";
import dts from "vite-plugin-dts";
import { defineConfig } from "vitest/config";

// Externalize all Node.js built-ins (with and without node: prefix)
const nodeBuiltins = new Set([...builtinModules, ...builtinModules.map((m) => `node:${m}`)]);

export default defineConfig({
  define: {
    "import.meta.vitest": "undefined",
  },
  build: {
    lib: {
      entry: resolve("export/index.ts"),
      fileName: "index",
      formats: ["es"],
    },
    outDir: "export_dist",
    emptyOutDir: true,
    sourcemap: true,
    rollupOptions: {
      external: (id: string) =>
        nodeBuiltins.has(id) ||
        (!id.startsWith(".") && !id.startsWith("/") && !id.startsWith("\0")),
    },
  },
  test: {
    testTimeout: 30000,
    watch: false,
    pool: "forks",
    maxWorkers: 1,
    isolate: false,
    passWithNoTests: true,
    coverage: {
      provider: "istanbul",
      reporter: ["lcovonly", "text"],
      reportsDirectory: "./coverage-vitest",
    },
    reporters: ["default", ["junit", { outputFile: "report-vitest.junit.xml" }]],
    detectAsyncLeaks: false,
    includeSource: ["export/**/*.ts"],
    include: ["tests/vitest/**/*.test.ts"],
  },
  plugins: [
    dts({
      staticImport: true,
      entryRoot: "export",
    }),
  ],
});
