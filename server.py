#!/usr/bin/env python3
"""Static server for the OpenKonung web build.

Serves the browser shell (index.html, app.js, manifest.json, openkonung.wasm,
konung.zip.pack) from `web/www` at the URL root. The original `.RES`/`.EXE`/
`Konung.N` files are served from the resolved install directory only for the
`?nopack=1` development fallback; a normal pack-only run never requests them.

    http://localhost:8000/

`--base` may be omitted: the server then probes `web/`, `web/assets/` and
`web/..` (the same order as the build) and logs the result. If no raw install
is present, it serves `web/www` only and the pack still works.
"""
from __future__ import annotations

import argparse
import http.server
import mimetypes
import os
import re
import socketserver
import sys
from functools import partial
from pathlib import Path

# The asset resolver lives beside the build scripts.
sys.path.insert(0, str(Path(__file__).resolve().parent / "tools"))
import assets as asset_tools  # noqa: E402


class _RangeReader:
    """File-like wrapper that exposes exactly `length` bytes from `start`."""

    def __init__(self, f, start: int, length: int):
        self._f = f
        self._left = length
        f.seek(start)

    def read(self, n: int = -1) -> bytes:
        if self._left <= 0:
            return b""
        if n < 0 or n > self._left:
            n = self._left
        data = self._f.read(n)
        self._left -= len(data)
        return data

    def close(self) -> None:
        self._f.close()


class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, base: Path, www: Path, **kwargs):
        self._base = base
        self._www = www
        super().__init__(*args, directory=str(base), **kwargs)

    def send_head(self):
        # Browser <video> needs byte ranges. Python's stdlib handler serves the
        # whole file only; add 206 responses for local development (production
        # servers already do this).
        path = self.translate_path(self.path)
        if not os.path.isfile(path):
            return super().send_head()
        match = re.match(r"^bytes=(\d*)-(\d*)$", self.headers.get("Range", ""))
        if not match or (not match.group(1) and not match.group(2)):
            return super().send_head()
        size = os.path.getsize(path)
        if match.group(1):
            start = int(match.group(1))
            end = int(match.group(2)) if match.group(2) else size - 1
        else:
            length = int(match.group(2))
            start = max(0, size - length)
            end = size - 1
        if start >= size or start > end:
            self.send_error(416, "Requested Range Not Satisfiable",
                            f"valid range: 0-{size - 1}")
            return None
        end = min(end, size - 1)
        length = end - start + 1
        f = open(path, "rb")
        self.send_response(206)
        self.send_header("Content-Type", mimetypes.guess_type(path)[0] or "application/octet-stream")
        self.send_header("Content-Range", f"bytes {start}-{end}/{size}")
        self.send_header("Content-Length", str(length))
        self.send_header("Accept-Ranges", "bytes")
        self.end_headers()
        return _RangeReader(f, start, length)

    def end_headers(self):
        self.send_header(
            "Cache-Control",
            "no-store, no-cache, must-revalidate, max-age=0"
        )
        self.send_header("Pragma", "no-cache")
        self.send_header("Expires", "0")
        super().end_headers()

    def translate_path(self, path: str) -> str:
        rel = path.split("?", 1)[0].split("#", 1)[0].lstrip("/")
        parts = [p for p in rel.split("/") if p not in ("", ".", "..")]
        rel = "/".join(parts)
        if rel == "":
            return str(self._www / "index.html")
        www_candidate = self._www / rel
        if www_candidate.is_file():
            return str(www_candidate)
        return str(self._base / rel)

    def log_message(self, *args):
        pass


def main() -> None:
    here = Path(__file__).resolve().parent
    ap = argparse.ArgumentParser(description="Serve the OpenKonung web build at /")
    ap.add_argument("--port", type=int, default=8000)
    ap.add_argument(
        "--base",
        type=Path,
        default=None,
        help="original install directory (default: auto-detect web/, web/assets/, web/..)",
    )
    ap.add_argument("--www", type=Path, default=here / "www", help="browser shell directory")
    args = ap.parse_args()

    www = args.www.resolve()
    if args.base is not None:
        base = args.base.resolve()
        print(f"[assets] using explicit install: {base}", file=sys.stderr)
    else:
        result = asset_tools.resolve(here, None)
        asset_tools.log_report(result)
        base = Path(result["base"]).resolve() if result["base"] else www
        if not result["base"]:
            print(
                "[assets] no raw install found; serving web/www only "
                "(the resource pack still works, ?nopack=1 will not).",
                file=sys.stderr,
            )

    handler = partial(Handler, base=base, www=www)

    socketserver.TCPServer.allow_reuse_address = True
    with socketserver.TCPServer(("", args.port), handler) as httpd:
        print(f"Serving OpenKonung at http://localhost:{args.port}/")
        print(f"  shell: {www}")
        print(f"  assets: {base}")
        httpd.serve_forever()


if __name__ == "__main__":
    main()
