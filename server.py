#!/usr/bin/env python3
"""Static server for the OpenKonung web build.

Serves the browser shell (index.html, app.js, manifest.json, openkonung.wasm)
from `web/www` at the URL root, and every other path (the original .RES/.EXE/
Konung.N assets) from the install directory. So the game lives at:

    http://localhost:8000/

while `manifest.base = "/"` still resolves straight to the original game files.
"""
from __future__ import annotations

import argparse
import http.server
import socketserver
from functools import partial
from pathlib import Path


class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, base: Path, www: Path, **kwargs):
        self._base = base
        self._www = www
        super().__init__(*args, directory=str(base), **kwargs)

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
    ap.add_argument("--base", type=Path, default=here.parent, help="original install directory")
    ap.add_argument("--www", type=Path, default=here / "www", help="browser shell directory")
    args = ap.parse_args()

    base = args.base.resolve()
    www = args.www.resolve()
    handler = partial(Handler, base=base, www=www)

    socketserver.TCPServer.allow_reuse_address = True
    with socketserver.TCPServer(("", args.port), handler) as httpd:
        print(f"Serving OpenKonung at http://localhost:{args.port}/")
        print(f"  shell: {www}")
        print(f"  assets: {base}")
        httpd.serve_forever()


if __name__ == "__main__":
    main()
