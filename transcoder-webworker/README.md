# Web worker for transcoding audio files

## Building

```sh
cargo build --target wasm32-unknown-unknown --release
wasm-bindgen target/wasm32-unknown-unknown/release/transcoder_webworker.wasm --out-dir ../web/assets/worker --target web
wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int --strip-debug -o ../web/assets/worker/transcoder_webworker_bg.wasm ../web/assets/worker/transcoder_webworker_bg.wasm
cp worker.js ../web/assets/worker/
```
