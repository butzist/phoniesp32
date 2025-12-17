// worker.js — an ES module Web Worker that loads the wasm-bindgen glue
// Adjust import path if your wasm-pack output is in a different place.
import init, { transcode } from "./transcoder_webworker.js";

// ensure init is only run once
let wasmReady = false;
let readyPromise = null;

async function ensureWasm() {
  if (wasmReady) return;
  if (!readyPromise) {
    // init() will fetch and instantiate the worker_crate_bg.wasm
    readyPromise = init();
    await readyPromise;
    wasmReady = true;
  } else {
    await readyPromise;
    wasmReady = true;
  }
}

self.onmessage = async (ev) => {
  let input = ev.data;

  if (!(input instanceof ArrayBuffer)) {
    console.error("worker: received unexpected message:", input);
    return;
  }

  let progress = (current, total) => {
    self.postMessage({progress: {current, total}});
  }

  try {
    await ensureWasm();
    const output = await transcode(input, progress);

    // Extract filename and data from the result object
    const filename = output.filename;
    const data = output.data;

    // transfer ownership of output buffer
    self.postMessage({result: {filename, data}}, [data]);
  } catch (err) {
    self.postMessage({error: String(err)});
  }
};

