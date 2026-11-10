#!/usr/bin/env python3
"""Transcode the original Konung AVI/Indeo-5 movies to browser MP4.

The shipped ``*.flm`` files are RIFF AVI containers with Intel Indeo 5
(``IV50``) video and PCM audio. Browsers cannot decode IV50, so the build
converts them once to H.264/AAC MP4 for ``<video>`` playback. The original
files remain untouched and are never shipped to ``www``.

Usage::

    python tools/transcode_videos.py --base ..
    python tools/transcode_videos.py --base .. --ffmpeg C:\\ffmpeg\\bin\\ffmpeg.exe
    python tools/transcode_videos.py --base .. --check

Outputs ``www/video/<lowercase>.mp4`` plus ``www/video/.transcode.json`` for
incremental rebuilds. A missing encoder or source directory is non-fatal unless
``--strict`` is given; the shell simply skips unavailable movies.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

VIDEO_SUBDIRS = (Path("Video"), Path("originalCD") / "Konung" / "Video")
EXPECTED = (
    "poster",
    "win",
    "gameover",
    "credits",
    *(f"defeat{n}" for n in (1, 4, 7, 11, 14, 16, 19, 22, 24, 27, 29, 33, 35, 37, 39)),
)
ENCODE_KEY = "h264-crf28-640x360-12fps-aac96"


def find_sources(base: Path) -> Path | None:
    for rel in VIDEO_SUBDIRS:
        candidate = base / rel
        if candidate.is_dir() and any(candidate.glob("*.flm")):
            return candidate
    return None


def find_ffmpeg(explicit: str | None) -> Path | None:
    candidates: list[Path] = []
    if explicit:
        candidates.append(Path(explicit))
    if os.environ.get("FFMPEG"):
        candidates.append(Path(os.environ["FFMPEG"]))
    which = shutil.which("ffmpeg")
    if which:
        candidates.append(Path(which))
    # Local development fallback: Documents/ffmpeg/*/bin/ffmpeg.exe.
    for pattern in ("ffmpeg-*/bin/ffmpeg.exe", "ffmpeg*/bin/ffmpeg.exe"):
        candidates.extend(sorted((Path.home() / "Documents" / "ffmpeg").glob(pattern)))
    for p in candidates:
        if p.is_file():
            return p
    return None


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def load_cache(path: Path) -> dict:
    try:
        return json.loads(path.read_text("utf-8"))
    except (OSError, ValueError):
        return {}


def main() -> int:
    here = Path(__file__).resolve().parent
    web = here.parent
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--base", type=Path, default=web.parent, help="install directory")
    ap.add_argument("--out", type=Path, default=web / "www" / "video", help="output directory")
    ap.add_argument("--ffmpeg", default=None, help="ffmpeg executable (default: env/PATH)")
    ap.add_argument("--check", action="store_true", help="only report what would be encoded")
    ap.add_argument("--strict", action="store_true", help="fail when a source/video is missing")
    args = ap.parse_args()

    base = args.base.resolve()
    src_dir = find_sources(base)
    if src_dir is None:
        msg = f"video source directory not found under {base}"
        if args.strict:
            print(f"error: {msg}", file=sys.stderr)
            return 1
        print(f"video: {msg}; skipping")
        return 0

    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=True)
    cache_path = out / ".transcode.json"
    cache = load_cache(cache_path)

    sources = {p.stem.lower(): p for p in src_dir.glob("*.flm") if p.is_file()}
    missing = [name for name in EXPECTED if name not in sources]
    if missing:
        msg = "missing source movie(s): " + ", ".join(missing)
        if args.strict:
            print(f"error: {msg}", file=sys.stderr)
            return 1
        print(f"video: {msg}")

    ffmpeg = find_ffmpeg(args.ffmpeg)
    if ffmpeg is None:
        msg = "ffmpeg not found (set FFMPEG or --ffmpeg)"
        if args.strict:
            print(f"error: {msg}", file=sys.stderr)
            return 1
        print(f"video: {msg}; skipping transcodes")
        return 0

    changed = 0
    for name, src in sorted(sources.items()):
        dst = out / f"{name}.mp4"
        key = f"{ENCODE_KEY}:{sha256(src)}"
        if dst.is_file() and cache.get(name) == key:
            continue
        if args.check:
            print(f"video: would transcode {src.name} -> {dst.name}")
            changed += 1
            continue
        # ffmpeg is invoked with -y and overwrites in place; never unlink the
        # previous output (the project keeps all generated files).
        cmd = [
            str(ffmpeg),
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-i",
            str(src),
            "-map",
            "0:v:0",
            "-map",
            "0:a:0?",
            "-vf",
            "scale=640:360:flags=lanczos",
            "-r",
            "12",
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-crf",
            "28",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-b:a",
            "96k",
            "-ac",
            "2",
            "-movflags",
            "+faststart",
            str(dst),
        ]
        print(f"video: {src.name} -> {dst.name}")
        proc = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        if proc.returncode != 0:
            print(f"error: ffmpeg failed for {src.name}: {proc.stderr.strip()}", file=sys.stderr)
            if args.strict:
                return 1
            continue
        cache[name] = key
        changed += 1

    cache_path.write_text(json.dumps(cache, indent=2, sort_keys=True), encoding="utf-8")
    print(f"video: {changed} encoded, {len(sources)} source movie(s) in {src_dir}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
