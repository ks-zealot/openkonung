#!/usr/bin/env bash
# Build the OpenKonung wasm module, the single-file resource pack and
# www/manifest.json.
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

# Resolve web/rust-toolchain.toml (pinned channel + wasm target) from here.
cd "$ROOT"

if command -v rustup >/dev/null 2>&1; then
    rustup target add wasm32-unknown-unknown >/dev/null 2>&1 || true
fi

echo "Building wasm (release)..."
cargo build --release --target wasm32-unknown-unknown --lib --manifest-path "$ROOT/Cargo.toml"

cp "$ROOT/target/wasm32-unknown-unknown/release/openkonung.wasm" "$ROOT/www/openkonung.wasm"

PYTHON="$(command -v python3 || command -v python)"
if [ -z "${PYTHON:-}" ]; then
    echo "error: python3 not found (needed to generate the manifest)." >&2
    exit 1
fi

RAW="$ROOT/www/manifest.raw.json"
PACK="$ROOT/www/openkonung.pack.zip"

echo "Transcoding videos..."
"$PYTHON" "$ROOT/tools/transcode_videos.py" --base "$BASE"

echo "Generating raw manifest..."
"$PYTHON" "$ROOT/tools/make_manifest.py" --base "$BASE" --out "$RAW"

echo "Packing resources (native examples/pack)..."
cargo run --release --example pack --manifest-path "$ROOT/Cargo.toml" -- \
    --base "$BASE" --manifest "$RAW" --out "$PACK"

echo "Generating final manifest (pack advertised, per-file fallback kept)..."
"$PYTHON" "$ROOT/tools/make_manifest.py" --base "$BASE" --pack "$PACK" --out "$ROOT/www/manifest.json"
rm -f "$RAW"

echo "Built web/www/openkonung.wasm, web/www/openkonung.pack.zip and web/www/manifest.json"
