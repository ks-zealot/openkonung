#!/usr/bin/env bash
# From-scratch Debian/Ubuntu bootstrap for the OpenKonung web build.
#
# Installs the base packages and, if necessary, Rust itself (via rustup),
# adds the wasm32 target, then builds the wasm module. Rust does NOT need to
# be installed beforehand.
#
# Usage:
#   bash web/setup.sh
#   bash web/serve.sh          # then open http://localhost:8000/
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

SUDO=""
if [ "$(id -u)" -ne 0 ]; then
    if command -v sudo >/dev/null 2>&1; then
        SUDO="sudo"
    else
        echo "warning: not root and sudo not found; package installation may fail." >&2
    fi
fi

if command -v apt-get >/dev/null 2>&1; then
    echo "==> Installing base packages (curl, build-essential, python3)..."
    $SUDO apt-get update -y
    $SUDO apt-get install -y --no-install-recommends \
        curl ca-certificates build-essential python3
else
    echo "warning: apt-get not found. Ensure curl, build-essential and python3 are installed." >&2
fi

if ! command -v cargo >/dev/null 2>&1 && ! [ -x "$HOME/.cargo/bin/cargo" ]; then
    echo "==> Installing Rust (rustup, minimal stable toolchain)..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --profile minimal --default-toolchain stable
else
    echo "==> Rust already present."
fi

# Make cargo/rustup available in this shell.
if [ -f "$HOME/.cargo/env" ]; then
    # shellcheck disable=SC1090
    . "$HOME/.cargo/env"
fi

echo "==> Adding wasm32-unknown-unknown target..."
rustup target add wasm32-unknown-unknown

chmod +x "$ROOT"/*.sh 2>/dev/null || true

echo "==> Building..."
bash "$ROOT/build.sh"

echo
echo "Done. Start the game with:"
echo "    bash web/serve.sh"
echo "Then open http://localhost:8000/"
