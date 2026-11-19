#!/usr/bin/env bash
# Build the WebGPU version into dist/ (static files: host anywhere).
#   one-time setup:  rustup target add wasm32-unknown-unknown
#                    cargo install wasm-bindgen-cli --version 0.2.130 --locked
#   build:           scripts/build-web.sh
#   preview locally: python -m http.server 8080 --directory dist
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --release --lib --target wasm32-unknown-unknown
rm -rf dist
mkdir -p dist/pkg
wasm-bindgen --target web --no-typescript --out-dir dist/pkg \
  target/wasm32-unknown-unknown/release/dioramas.wasm
cp web/* dist/
echo "Built dist/ ($(du -sh dist | cut -f1))"
