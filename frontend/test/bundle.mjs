// Bundles the dashboard as one script a simulated browser can run. uPlot needs a real canvas,
// so a stand-in is used here: these tests are about what the page SAYS, not about pixels.
import { build } from "vite";
import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, "..");

export async function bundleApp() {
  const out = mkdtempSync(path.join(tmpdir(), "monitoring-ui-"));
  await build({
    configFile: false,
    root,
    logLevel: "silent",
    resolve: {
      alias: [
        { find: "uplot/dist/uPlot.min.css", replacement: path.join(here, "empty.css") },
        { find: /^uplot$/, replacement: path.join(here, "uplot-stub.js") },
      ],
    },
    build: {
      outDir: out,
      emptyOutDir: true,
      lib: { entry: path.join(root, "src/main.ts"), formats: ["iife"], name: "App", fileName: () => "app.iife.js" },
    },
  });
  return readFileSync(path.join(out, "app.iife.js"), "utf8");
}
