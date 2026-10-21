#!/usr/bin/env bash
# Builds the HTML5 version into dist/web (upload this folder / zip to itch.io).
set -euo pipefail
cd "$(dirname "$0")/.."

rustup target add wasm32-unknown-unknown >/dev/null 2>&1 || true
cargo build --release --target wasm32-unknown-unknown

rm -rf dist/web
mkdir -p dist/web
cp web/index.html web/mq_js_bundle.js dist/web/
cp target/wasm32-unknown-unknown/release/rusty-scape.wasm dist/web/

# Optional size optimization if binaryen's wasm-opt is installed.
if command -v wasm-opt >/dev/null 2>&1; then
    wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
        dist/web/rusty-scape.wasm -o dist/web/rusty-scape.wasm
fi

echo "Web build ready in dist/web ($(du -sh dist/web | cut -f1))."
echo "Test locally with: python3 -m http.server -d dist/web 8080"
