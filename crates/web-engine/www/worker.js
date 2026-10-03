// The engine runs here, off the page's thread, which a browser never lets block.
import init, { slice, memoryBytes } from "./pkg/web_engine.js";

const ready = init();

onmessage = async ({ data }) => {
  await ready;
  try {
    const started = performance.now();
    const sliced = slice(data.bytes, data.budget, Date.now() / 1000);
    const seconds = (performance.now() - started) / 1000;
    const bytes = sliced.takeBytes();
    postMessage(
      { name: data.name, extension: sliced.extension, bytes, seconds, memory: memoryBytes() },
      [bytes.buffer],
    );
  } catch (error) {
    postMessage({ error: String(error) });
  }
};
