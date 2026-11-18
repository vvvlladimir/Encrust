// node run.mjs <site dir> <model file in it>…: serves the one directory the spike's page
// lives in, with no COOP/COEP headers as an MCP Apps page is served, and times its prepare
// of each model in Chromium. See prototypes/connector/README.md.
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { basename, extname, join } from "node:path";
import { createRequire } from "node:module";

// Playwright is installed globally, which an ES module does not search.
const { chromium } = createRequire("/opt/node-tools/node_modules/")("playwright");

const [site, ...models] = process.argv.slice(2);
const types = { ".html": "text/html", ".js": "text/javascript", ".wasm": "application/wasm" };
const server = createServer(async (request, response) => {
  const asked = new URL(request.url, "http://localhost").pathname;
  // Only files in the site directory and its `pkg`, by name: nothing above it is served.
  const parts = asked.split("/").filter(Boolean);
  const inside = parts.length === 1 || (parts.length === 2 && parts[0] === "pkg");
  if (!inside || parts.some((part) => part !== basename(part) || part.startsWith("."))) {
    response.writeHead(404).end();
    return;
  }
  try {
    const body = await readFile(join(site, ...parts));
    response.writeHead(200, { "content-type": types[extname(asked)] ?? "application/octet-stream" });
    response.end(body);
  } catch {
    response.writeHead(404).end();
  }
});
await new Promise((ready) => server.listen(0, "127.0.0.1", ready));
const port = server.address().port;
const browser = await chromium.launch();
for (const model of models) {
  const page = await browser.newPage();
  await page.goto(`http://127.0.0.1:${port}/measure.html?model=${encodeURIComponent(model)}`);
  await page.waitForFunction(() => window.result !== undefined, null, { timeout: 30 * 60_000 });
  console.log(JSON.stringify({ model, ...(await page.evaluate(() => window.result)) }));
  await page.close();
}
await browser.close();
server.close();
