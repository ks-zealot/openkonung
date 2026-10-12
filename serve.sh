#!/usr/bin/env bash
# Serve the OpenKonung web build at http://localhost:<port>/
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE="$(cd "$ROOT/.." && pwd)"
PORT="${1:-8000}"

PYTHON="$(command -v python3 || command -v python || true)"
if [ -z "$PYTHON" ]; then
    echo "error: python3 not found." >&2
    exit 1
fi

echo "Open: http://localhost:${PORT}/"
exec "$PYTHON" "$ROOT/server.py" --port "$PORT" --base "$BASE" --www "$ROOT/www"
