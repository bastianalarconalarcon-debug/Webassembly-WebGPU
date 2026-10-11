#!/usr/bin/env sh
# Compila el juego (Rust -> WebAssembly) y lo deja en web/juego.wasm.
set -e
cd "$(dirname "$0")"
cargo build --release --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/juego.wasm web/juego.wasm
ls -lh web/juego.wasm
