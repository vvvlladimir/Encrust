// The browser's run under Node's V8: time and peak wasm memory of one project.
// node measure.mjs <pkg dir> <project.encrust> [budget MB]; see docs/design/web-build.md.
import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const [pkg, project, budget = "1024"] = process.argv.slice(2);
const engine = await import(pathToFileURL(resolve(pkg, "web_engine.js")));
engine.initSync({ module: readFileSync(join(pkg, "web_engine_bg.wasm")) });

const bytes = readFileSync(project);
const started = performance.now();
const sliced = engine.slice(bytes, Number(budget), Date.now() / 1000);
const seconds = (performance.now() - started) / 1000;
const output = sliced.takeBytes();

console.log(JSON.stringify({
  seconds: Number(seconds.toFixed(2)),
  peak_wasm_mb: Math.round(engine.memoryBytes() / 1e6),
  output_mb: Number((output.length / 1e6).toFixed(1)),
  extension: sliced.extension,
}));
