#!/usr/bin/env bash
# Build the OpenKonung wasm module and regenerate www/manifest.json.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE="$(cd "$ROOT/.." && pwd)"

# Pick up a rustup install if the shell has not sourced it yet.
if ! command -v cargo >/dev/null 2>&1 && [ -f "$HOME/.cargo/env" ]; then
    # shellcheck disable=SC1090
    . "$HOME/.cargo/env"
fi

if ! command -v cargo >/dev/null 2>&1; then
    echo "error: cargo not found. Run web/setup.sh first." >&2
    exit 1
fi

if command -v rustup >/dev/null 2>&1; then
    rustup target add wasm32-unknown-unknown >/dev/null 2>&1 || true
fi

echo "Building wasm (release)..."
cargo build --release --target wasm32-unknown-unknown --manifest-path "$ROOT/Cargo.toml"

cp "$ROOT/target/wasm32-unknown-unknown/release/openkonung.wasm" "$ROOT/www/openkonung.wasm"

PYTHON="$(command -v python3 || command -v python)"
if [ -z "${PYTHON:-}" ]; then
    echo "error: python3 not found (needed to generate the manifest)." >&2
    exit 1
fi
"$PYTHON" "$ROOT/tools/make_manifest.py" --base "$BASE" --out "$ROOT/www/manifest.json"

echo "Built web/www/openkonung.wasm and web/www/manifest.json"
