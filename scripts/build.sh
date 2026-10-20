#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
if ! command -v wasm-bindgen >/dev/null || [[ "$(wasm-bindgen --version)" != "wasm-bindgen 0.2.121" ]]; then
  echo "Install the matching tool: cargo install wasm-bindgen-cli --version 0.2.121 --locked"
  exit 1
fi
cargo rustc --locked --lib --release --target wasm32-unknown-unknown -- -C link-arg=-zstack-size=8388608
mkdir -p dist/wasm
wasm-bindgen --target web --out-dir dist/wasm "${CARGO_TARGET_DIR:-target}/wasm32-unknown-unknown/release/traceglass.wasm"
cp web/index.html web/app.js web/style.css web/_headers dist/
cp web/sample.jsonl dist/
cp fonts/OFL.txt dist/font-license.txt
cp GPUI-LICENSE-APACHE dist/gpui-license.txt
cp LICENSE-LUCIDE dist/icon-license.txt
cp THIRD-PARTY-NOTICES.md dist/third-party-notices.txt
wc -c dist/wasm/traceglass_bg.wasm
