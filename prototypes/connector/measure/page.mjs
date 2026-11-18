// node page.mjs <site dir> <model file in it>: the spike's page in Chromium with a stand-in
// for ChatGPT's `window.openai`, to check the page's own code before it runs in ChatGPT.
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { basename, extname, join } from "node:path";
import { createRequire } from "node:module";

const { chromium } = createRequire("/opt/node-tools/node_modules/")("playwright");

const [site, model] = process.argv.slice(2);
const types = { ".html": "text/html", ".js": "text/javascript", ".wasm": "application/wasm" };
const logged = [];
const server = createServer(async (request, response) => {
  const asked = new URL(request.url, "http://localhost").pathname;
  if (asked === "/log") {
    let body = "";
    for await (const chunk of request) body += chunk;
    logged.push(JSON.parse(body));
    response.writeHead(204).end();
    return;
  }
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
const origin = `http://127.0.0.1:${server.address().port}`;
const browser = await chromium.launch();
const page = await browser.newPage();
await page.addInitScript((given) => {
  window.called = [];
  window.openai = {
    toolOutput: given,
    callTool: async (name, args) => window.called.push({ name, args }),
    sendFollowUpMessage: async (message) => window.called.push({ followUp: message.prompt }),
  };
}, {
  download_url: `${origin}/${model}`,
  file_id: "file-1",
  engine: `${origin}/pkg/connector_engine.js`,
  printer_toml: `${origin}/printer.toml`,
  resin_toml: `${origin}/resin.toml`,
});
await page.goto(`${origin}/connector.html`);
await page.waitForSelector("#download:not([hidden])", { timeout: 10 * 60_000 });
console.log(JSON.stringify({
  answers: await page.$$eval("#answers li", (items) => items.map((item) => item.textContent)),
  called: await page.evaluate(() => window.called.map((call) => call.name ?? "followUp")),
  logged: logged.length,
}, null, 2));
await browser.close();
server.close();
