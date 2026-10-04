// A thread of the window: a worker that brings the module up on its shared memory, runs
// one task and ends. This file is both the page's way to start one and the worker itself,
// and the way back to the page for what only the page may do.

// Rust's own threads get 2 MiB; the stack must be a whole number of 64 KiB pages.
const STACK_BYTES = 2 << 20;
const NAME = "encrust-thread";

const IN_WORKER = typeof WorkerGlobalScope !== "undefined" && self.name === NAME;

// Every thread is started by the page. A worker's own child starts only once that worker
// returns to its event loop, which one waiting for its pool's threads never does.
export function startThread(bindings, module, memory) {
  if (IN_WORKER) {
    self.postMessage({ start: { bindings, module, memory } });
    return;
  }
  const worker = new Worker(new URL(import.meta.url), { type: "module", name: NAME });
  worker.onmessage = ({ data }) => {
    if (data.start) {
      startThread(data.start.bindings, data.start.module, data.start.memory);
    } else if (data.offer) {
      offerFile(data.offer.name, data.offer.blob);
    }
  };
  worker.postMessage({ bindings, module, memory });
}

// Hands a file to the user as a download, which only the page can start.
export function offerFile(name, blob) {
  if (IN_WORKER) {
    self.postMessage({ offer: { name, blob } });
    return;
  }
  const link = document.createElement("a");
  link.href = URL.createObjectURL(blob);
  link.download = name;
  link.click();
  // Long after the browser has taken what it needs from it.
  setTimeout(() => URL.revokeObjectURL(link.href), 60_000);
}

if (IN_WORKER) {
  self.onmessage = async ({ data }) => {
    try {
      const bindings = await import(data.bindings);
      const exports = bindings.initSync({
        module: data.module,
        memory: data.memory,
        thread_stack_size: STACK_BYTES,
      });
      await bindings.encrustRunThread();
      // Hands the stack and thread-locals back to the shared allocator.
      exports.__wbindgen_thread_destroy();
    } catch (error) {
      // A panic ends here: the page has no other way to hear of it.
      console.error("encrust thread:", error);
    }
    close();
  };
}
