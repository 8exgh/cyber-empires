#!/usr/bin/env bash
# Build the WebAssembly module and place it next to the web shell.
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"
rustup target list --installed | grep -q wasm32-unknown-unknown || rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/cyber_empires.wasm web/cyber_empires.wasm
ls -la web/cyber_empires.wasm
echo "Serve the web/ folder, e.g.:  python3 -m http.server 8080 -d web"
