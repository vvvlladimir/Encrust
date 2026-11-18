// The page's service worker. It puts the two isolation headers on every response for a host
// that cannot send them, and keeps this build's files so the window opens offline.
// `cargo xtask web` writes BUILD and FILES in; see docs/design/web-build.md.
const BUILD = "__BUILD__";
const FILES = __FILES__;
const CACHE = `encrust-${BUILD}`;

self.addEventListener("install", (event) => {
  event.waitUntil((async () => {
    // A build that cannot be kept still has to isolate the page, so a failure here only
    // costs working offline.
    try {
      const cache = await caches.open(CACHE);
      // The page keeps the module once it has it, so it is never fetched twice (ADR 0399).
      const small = FILES.filter((file) => !file.endsWith(".wasm"));
      // "no-store": "reload" rewrote the HTTP cache entry the page was still reading, and
      // Chromium aborted the page's copy (ERR_CACHE_WRITE_FAILURE).
      await cache.addAll(small.map((file) => new Request(file, { cache: "no-store" })));
    } catch (error) {
      console.warn("Encrust will not open offline:", error);
    }
    // The first worker takes the page at once, which is what isolates it. A later build
    // waits until a starting page asks it to take over (below).
    if (!self.registration.active) {
      await self.skipWaiting();
    }
  })());
});

// A page asks before it starts. The new build takes over only if that page is the build's
// one tab: a running tab's threads import the old script (ADR 0399).
self.addEventListener("message", (event) => {
  if (event.data !== "take-over") {
    return;
  }
  event.waitUntil((async () => {
    const windows = await self.clients.matchAll({ type: "window", includeUncontrolled: true });
    const alone = windows.filter((client) => client.url.startsWith(self.registration.scope))
      .length <= 1;
    event.ports[0]?.postMessage(alone);
    if (alone) {
      await self.skipWaiting();
    }
  })());
});

self.addEventListener("activate", (event) => {
  event.waitUntil((async () => {
    for (const key of await caches.keys()) {
      if (key.startsWith("encrust-") && key !== CACHE) {
        await caches.delete(key);
      }
    }
    await self.clients.claim();
  })());
});

self.addEventListener("fetch", (event) => {
  const request = event.request;
  if (request.method !== "GET" || new URL(request.url).origin !== self.location.origin) {
    return;
  }
  event.respondWith((async () => {
    const cached = await caches.match(request, { cacheName: CACHE, ignoreSearch: true });
    return isolated(cached ?? await fetch(request));
  })());
});

function isolated(response) {
  if (response.status === 0) {
    return response;
  }
  const headers = new Headers(response.headers);
  headers.set("Cross-Origin-Opener-Policy", "same-origin");
  headers.set("Cross-Origin-Embedder-Policy", "require-corp");
  return new Response(response.body, {
    status: response.status,
    statusText: response.statusText,
    headers,
  });
}
