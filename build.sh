#!/usr/bin/env bash
# Build the OpenKonung wasm module, the single-file resource pack
# (www/konung.zip.pack) and www/manifest.json.
#
# First build: reads the original files from ./, assets/ or ../
# Later builds: reuse www/konung.zip.pack and never touch .res/.exe.
# Pass --repack to rebuild the archive after changing the original assets.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPACK=0
NOVIDEOS=0
ASSETS=""

while [ $# -gt 0 ]; do
    case "$1" in
        --repack) REPACK=1 ;;
        --no-videos) NOVIDEOS=1 ;;
        --assets) shift; ASSETS="${1:-}" ;;
        -h|--help)
            echo "usage: build.sh [--repack] [--no-videos] [--assets <install-dir>]"
            exit 0
            ;;
        *) echo "error: unknown argument '$1'" >&2; exit 2 ;;
    esac
    shift
done

# Pick up a rustup install if the shell has not sourced it yet.
if ! command -v cargo >/dev/null 2>&1 && [ -f "$HOME/.cargo/env" ]; then
    # shellcheck disable=SC1090
    . "$HOME/.cargo/env"
fi

if ! command -v cargo >/dev/null 2>&1; then
    echo "error: cargo not found. Run setup.sh first." >&2
    exit 1
fi

PYTHON="$(command -v python3 || command -v python)"
if [ -z "${PYTHON:-}" ]; then
    echo "error: python3 not found (needed for asset discovery and the manifest)." >&2
    exit 1
fi

# Resolve web/rust-toolchain.toml (pinned channel + wasm target) from here.
cd "$ROOT"

if command -v rustup >/dev/null 2>&1; then
    rustup target add wasm32-unknown-unknown >/dev/null 2>&1 || true
fi

PACK="$ROOT/www/konung.zip.pack"
LEGACY="$ROOT/www/openkonung.pack.zip"

# Migrate an existing legacy pack by copying it; never delete the original.
if [ ! -f "$PACK" ] && [ -f "$LEGACY" ]; then
    cp "$LEGACY" "$PACK"
    echo "[build] copied legacy pack to www/konung.zip.pack"
fi

NEED_PACK=0
if [ "$REPACK" -eq 1 ] || [ ! -f "$PACK" ]; then
    NEED_PACK=1
fi

BASE=""
if [ "$NEED_PACK" -eq 1 ]; then
    ASSET_ARGS=(--repo-root "$ROOT" --print-base)
    if [ -n "$ASSETS" ]; then
        ASSET_ARGS+=(--assets "$ASSETS")
    fi
    BASE="$("$PYTHON" "$ROOT/tools/assets.py" "${ASSET_ARGS[@]}" | tail -n 1)"
    if [ -z "$BASE" ]; then
        echo "error: no original files found; cannot build the resource pack." >&2
        echo "       Put them in assets/ or ./ (or pass --assets <dir>)." >&2
        exit 2
    fi
fi

echo "Building wasm (release)..."
cargo build --release --target wasm32-unknown-unknown --lib --manifest-path "$ROOT/Cargo.toml"

cp "$ROOT/target/wasm32-unknown-unknown/release/openkonung.wasm" "$ROOT/www/openkonung.wasm"

if [ "$NEED_PACK" -eq 1 ]; then
    if [ "$NOVIDEOS" -eq 0 ]; then
        echo "Transcoding videos..."
        "$PYTHON" "$ROOT/tools/transcode_videos.py" --base "$BASE"
    else
        echo "Skipping video transcode (--no-videos)."
    fi

    RAW="$ROOT/www/manifest.raw.json"
    echo "Generating raw manifest..."
    "$PYTHON" "$ROOT/tools/make_manifest.py" --base "$BASE" --out "$RAW"

    echo "Packing resources (native examples/pack)..."
    cargo run --release --example pack --manifest-path "$ROOT/Cargo.toml" -- \
        --base "$BASE" --manifest "$RAW" --out "$PACK"

    echo "Generating final manifest (pack advertised, per-file fallback kept)..."
    "$PYTHON" "$ROOT/tools/make_manifest.py" --base "$BASE" --pack "$PACK" --out "$ROOT/www/manifest.json"
else
    echo "[build] reusing pack: $PACK (pass --repack to rebuild it)"
    "$PYTHON" "$ROOT/tools/make_manifest.py" --from-pack "$PACK" --out "$ROOT/www/manifest.json"
fi

echo "Built www/openkonung.wasm, www/konung.zip.pack and www/manifest.json"
