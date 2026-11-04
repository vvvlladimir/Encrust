// Opens the browser build in Chromium, drops a cube on it and fails unless the window draws it
// with no shader, GPU or start-up error. See docs/design/web-build.md, "Checking a build".
//
// node smoke.mjs --dist target/web/dist          a build on disk, served here with its headers
// node smoke.mjs --url https://encrust.app/app/run/ [--at-least v0.7.0 --wait-minutes 45]
//   [--out DIR]                                   where the log and the pictures go
import { createServer } from 'node:http';
import { mkdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { extname, join, normalize } from 'node:path';
import { chromium } from 'playwright';

const args = Object.fromEntries(
  process.argv.slice(2).reduce((pairs, word, i, all) =>
    word.startsWith('--') ? [...pairs, [word.slice(2), all[i + 1]]] : pairs, []));
const OUT = args.out ?? 'web-smoke';
mkdirSync(OUT, { recursive: true });
rmSync(join(OUT, 'failures.txt'), { force: true });

// What a broken window says in the console: a shader the browser refuses, a pipeline or frame
// made invalid by one, a Rust panic, a trap, a worker that died.
const FATAL = /WGSL|Invalid (ShaderModule|RenderPipeline|CommandBuffer|BindGroup|Texture)|panicked at|RuntimeError|unreachable|device (was )?lost|encrust thread:/i;
const lines = [];
const failures = [];
const started = Date.now();
const say = (line) => {
  const stamped = `[${((Date.now() - started) / 1000).toFixed(1)}s] ${line}`;
  lines.push(stamped);
  console.log(stamped.slice(0, 400));
};
const fail = (why) => { failures.push(why); say(`FAIL ${why}`); };

const server = args.dist ? await serve(args.dist) : null;
const url = server ? `http://127.0.0.1:${server.address().port}/` : args.url;
if (!url) throw new Error('give --dist DIR or --url URL');

try {
  if (args['at-least']) await waitForVersion(url, args['at-least'], Number(args['wait-minutes'] ?? 0));
  if (!failures.length) await open(url);
} finally {
  server?.close();
  writeFileSync(join(OUT, 'console.log'), lines.join('\n') + '\n');
}
if (failures.length) {
  writeFileSync(join(OUT, 'failures.txt'), failures.join('\n') + '\n');
  process.exit(1);
}
say('the web window opens and draws a model');

async function open(url) {
  // The full browser: the headless shell draws no WebGPU canvas. Linux runs it with a window on
  // the virtual display xvfb-run gives it, where SwiftShader's frames reach the screenshot.
  const browser = await chromium.launch({
    channel: 'chromium',
    headless: process.platform !== 'linux',
    args: gpuFlags(),
  });
  const context = await browser.newContext({ viewport: { width: 1280, height: 800 } });
  const page = await context.newPage();
  page.on('console', (message) => {
    const text = message.text();
    if (['error', 'warning'].includes(message.type())) say(`console.${message.type()}: ${text}`);
    if (FATAL.test(text)) fail(`console: ${text.split('\n')[0]}`);
  });
  page.on('pageerror', (error) => fail(`page error: ${error.message}`));
  page.on('crash', () => fail('the page crashed'));
  page.on('response', (response) => {
    if (response.status() >= 400) fail(`HTTP ${response.status()} for ${response.url()}`);
  });

  await page.goto(url, { waitUntil: 'load', timeout: 60_000 });
  const adapter = await page.evaluate(async () => {
    const found = await navigator.gpu?.requestAdapter();
    return found && `${found.info.vendor} ${found.info.architecture} ${found.info.description}`.trim();
  });
  say(`WebGPU adapter: ${adapter ?? 'none'}`);
  if (adapter === undefined || adapter === null) fail('this browser has no WebGPU adapter: the runner, not the build');
  // The module is 13 MB; the start page is up once the canvas has drawn more than one colour.
  const start = await settle(page, 'start', 90_000);
  const notice = await page.locator('#notice').innerText().catch(() => '');
  if (notice.trim()) fail(`the page says: ${notice.trim()}`);
  if (!start.drawn) fail('the start page never drew');

  await dropCube(page);
  await page.waitForTimeout(5_000);
  const plate = await settle(page, 'plate', 30_000);
  if (!plate.drawn) fail('the plate is blank after a model was dropped');
  const changed = await difference(page, start.png, plate.png);
  say(`after the drop ${(changed * 100).toFixed(1)} % of the window changed`);
  if (changed < 0.05) fail('dropping a model changed nothing on screen');
  const alive = await Promise.race([
    page.evaluate(() => true),
    new Promise((done) => setTimeout(() => done(false), 5_000)),
  ]);
  if (!alive) fail("the page's thread does not answer");
  if (failures.length) await describeGpu(context);
  await browser.close();
}

// Linux runners have no GPU: SwiftShader on the system's Vulkan loader is the adapter there, and
// without a Vulkan surface, which it cannot present to; see docs/design/web-build.md.
function gpuFlags() {
  const flags = ['--enable-unsafe-webgpu', '--ignore-gpu-blocklist'];
  return process.platform === 'linux'
    ? [...flags, '--enable-features=Vulkan', '--use-angle=vulkan', '--use-vulkan=swiftshader',
      '--use-webgpu-adapter=swiftshader', '--disable-vulkan-surface', '--enable-unsafe-swiftshader']
    : flags;
}

// What the browser made of the GPU, kept beside the log for a failure the runner may cause.
async function describeGpu(context) {
  const page = await context.newPage();
  try {
    await page.goto('chrome://gpu', { timeout: 15_000 });
    await page.waitForTimeout(2_000);
    writeFileSync(join(OUT, 'gpu.txt'), await page.locator('body').innerText());
  } catch (error) {
    say(`chrome://gpu unreadable: ${error.message}`);
  }
}

// Waits until the window shows more than a flat fill, and keeps that picture.
async function settle(page, name, timeoutMs) {
  const until = Date.now() + timeoutMs;
  let png;
  let colours = 0;
  while (Date.now() < until) {
    png = await page.screenshot({ timeout: 10_000 });
    colours = await colourCount(page, png);
    if (colours > 32) break;
    await page.waitForTimeout(2_000);
  }
  writeFileSync(join(OUT, `${name}.png`), png);
  say(`${name}: ${colours} colours`);
  return { png, drawn: colours > 32 };
}

// Pictures are decoded by the browser itself, on a page of their own, so nothing else is needed.
async function pixels(page, pngs) {
  const scratch = await page.context().newPage();
  const result = await scratch.evaluate(async (encoded) => Promise.all(encoded.map(async (b64) => {
    const bitmap = await createImageBitmap(await (await fetch(`data:image/png;base64,${b64}`)).blob());
    const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
    const context = canvas.getContext('2d');
    context.drawImage(bitmap, 0, 0);
    return Array.from(context.getImageData(0, 0, bitmap.width, bitmap.height).data);
  })), pngs.map((png) => png.toString('base64')));
  await scratch.close();
  return result;
}

async function colourCount(page, png) {
  const [data] = await pixels(page, [png]);
  const seen = new Set();
  for (let i = 0; i < data.length; i += 4 * 37) seen.add((data[i] << 16) | (data[i + 1] << 8) | data[i + 2]);
  return seen.size;
}

async function difference(page, before, after) {
  const [a, b] = await pixels(page, [before, after]);
  let changed = 0;
  for (let i = 0; i < a.length; i += 4) {
    if (Math.abs(a[i] - b[i]) + Math.abs(a[i + 1] - b[i + 1]) + Math.abs(a[i + 2] - b[i + 2]) > 24) changed++;
  }
  return changed / (a.length / 4);
}

// A 20 mm cube as a binary STL, dropped on the canvas the way a user drops a file.
async function dropCube(page) {
  const stl = cubeStl(20);
  const dataTransfer = await page.evaluateHandle((bytes) => {
    const transfer = new DataTransfer();
    transfer.items.add(new File([new Uint8Array(bytes)], 'smoke-cube.stl', { type: 'model/stl' }));
    return transfer;
  }, Array.from(stl));
  for (const type of ['dragenter', 'dragover', 'drop']) {
    await page.dispatchEvent('canvas', type, { dataTransfer });
  }
  say('dropped smoke-cube.stl');
}

function cubeStl(size) {
  const corner = (i) => [(i & 1) * size, ((i >> 1) & 1) * size, ((i >> 2) & 1) * size];
  const faces = [[0, 2, 1], [1, 2, 3], [4, 5, 6], [5, 7, 6], [0, 1, 4], [1, 5, 4],
    [2, 6, 3], [3, 6, 7], [0, 4, 2], [2, 4, 6], [1, 3, 5], [3, 7, 5]];
  const bytes = Buffer.alloc(84 + faces.length * 50);
  bytes.writeUInt32LE(faces.length, 80);
  faces.forEach((face, f) => face.forEach((v, k) => corner(v).forEach((c, axis) =>
    bytes.writeFloatLE(c, 84 + f * 50 + 12 + k * 12 + axis * 4))));
  return bytes;
}

// The build names its version in sw.js; a rebuild of the same version only changes the hash
// after it, so the live build passes once its version is at least the release's.
async function waitForVersion(url, wanted, minutes) {
  const until = Date.now() + minutes * 60_000;
  for (;;) {
    const live = await liveVersion(url).catch((error) => `unreadable (${error.message})`);
    if (/^\d/.test(live) && compare(live, wanted) >= 0) {
      say(`live build ${live}, at least ${wanted}`);
      return;
    }
    if (Date.now() >= until) {
      fail(`the live build is ${live}, older than release ${wanted}`);
      return;
    }
    say(`live build ${live}, waiting for ${wanted}`);
    await new Promise((done) => setTimeout(done, 60_000));
  }
}

async function liveVersion(url) {
  const response = await fetch(new URL('sw.js', url), { cache: 'no-store' });
  if (!response.ok) throw new Error(`HTTP ${response.status}`);
  const build = (await response.text()).match(/const BUILD = "([^"]+)"/)?.[1];
  if (!build) throw new Error('sw.js names no build');
  return build.replace(/-[0-9a-f]{16}$/, '');
}

function compare(a, b) {
  const parts = (v) => v.replace(/^v/, '').split(/[.-]/).slice(0, 3).map(Number);
  const [x, y] = [parts(a), parts(b)];
  for (let i = 0; i < 3; i++) if (x[i] !== y[i]) return x[i] - y[i];
  return 0;
}

// A static server sending what the host sends: the two isolation headers, and no caching.
async function serve(root) {
  const types = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm',
    '.json': 'application/json', '.webmanifest': 'application/manifest+json', '.svg': 'image/svg+xml',
    '.png': 'image/png', '.md': 'text/markdown' };
  const server = createServer((request, response) => {
    let path = normalize(join(root, decodeURIComponent(new URL(request.url, 'http://x').pathname)));
    if (!path.startsWith(normalize(root))) return response.writeHead(403).end();
    try {
      if (statSync(path).isDirectory()) path = join(path, 'index.html');
      response.writeHead(200, {
        'Content-Type': types[extname(path)] ?? 'application/octet-stream',
        'Cross-Origin-Opener-Policy': 'same-origin',
        'Cross-Origin-Embedder-Policy': 'require-corp',
        'Cache-Control': 'no-cache',
      }).end(readFileSync(path));
    } catch {
      response.writeHead(404).end();
    }
  });
  await new Promise((done) => server.listen(0, '127.0.0.1', done));
  return server;
}
