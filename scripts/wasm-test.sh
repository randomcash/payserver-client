#!/usr/bin/env bash
# Run the wasm-bindgen-test suite: the tests tagged `#[wasm_bindgen_test]`
# that need a real JS object graph (js_sys::Object / js_sys::Reflect) rather
# than a mocked EIP-1193 provider, so a plain `#[test]` can't reach them.
#
# None of these tests touch the DOM, so they run under Node.js rather than a
# headless browser - `wasm-bindgen-test`'s default when
# `wasm_bindgen_test_configure!(run_in_browser)` is not called. CI uses this
# same script so a local run and CI can't drift apart.
#
# Needs a recent Node - 18 crashes partway through the run (the runtime's glue
# JS uses a WASM externref feature Node 18's V8 doesn't have); Node 22, what
# the layout job already installs, works.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

# wasm-bindgen-test-runner has to match the wasm-bindgen version this crate
# builds against exactly, or it refuses to run the test binary at all.
version="$(grep -m1 -A1 '^name = "wasm-bindgen"$' Cargo.lock | grep -m1 -oP 'version = "\K[^"]+')"
[ -n "$version" ] || { echo "::error::no wasm-bindgen version found in Cargo.lock" >&2; exit 1; }

command -v node >/dev/null || { echo "::error::node is required to run wasm-bindgen-test" >&2; exit 1; }

if ! command -v wasm-bindgen-test-runner >/dev/null || \
   [ "$(wasm-bindgen-test-runner --version 2>/dev/null | grep -oP '[\d.]+$')" != "$version" ]; then
  cargo install wasm-bindgen-cli --version "$version" --locked
fi

rustup target add wasm32-unknown-unknown >/dev/null

export CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner
cargo test --target wasm32-unknown-unknown --lib
