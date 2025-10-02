CRATE          := transcoder_webworker
TARGET         := wasm32-unknown-unknown
PROFILE        := release
TARGET_DIR     := target/$(TARGET)/$(PROFILE)
OUT_DIR        := ../web/assets/worker
WORKER_JS_SRC  := worker.js

WASM_BINDGEN   := wasm-bindgen
WASM_OPT       := wasm-opt
WASM_OPT_FLAGS := -Oz --enable-bulk-memory --enable-nontrapping-float-to-int --strip-debug

# Default target
all: $(OUT_DIR)/worker.js $(OUT_DIR)/$(CRATE)_bg.wasm $(OUT_DIR)/$(CRATE).js

$(TARGET_DIR)/$(CRATE).wasm:
	cargo build --target $(TARGET) --release

$(TARGET_DIR)/$(CRATE)_bg.wasm $(TARGET_DIR)/$(CRATE).js: $(TARGET_DIR)/$(CRATE).wasm
	$(WASM_BINDGEN) $(TARGET_DIR)/$(CRATE).wasm --out-dir $(TARGET_DIR) --target web

$(TARGET_DIR)/$(CRATE)_bg.opt.wasm: $(TARGET_DIR)/$(CRATE)_bg.wasm
	$(WASM_OPT) $(WASM_OPT_FLAGS) -o $@ $<

$(OUT_DIR)/$(CRATE)_bg.wasm: $(TARGET_DIR)/$(CRATE)_bg.opt.wasm
	cp $< $@

$(OUT_DIR)/$(CRATE).js: $(TARGET_DIR)/$(CRATE).js
	cp $< $@

$(OUT_DIR)/worker.js: $(WORKER_JS_SRC)
	cp $(WORKER_JS_SRC) $(OUT_DIR)/

# Cleanup
clean:
	cargo clean
	rm -rf $(OUT_DIR)/*
	rm -rf $(TARGET_DIR)/*

.PHONY: all clean
