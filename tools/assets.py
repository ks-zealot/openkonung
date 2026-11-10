#!/usr/bin/env python3
"""Locate the original Konung install files and the resource pack.

The git repository root is ``web/``.  A fresh clone may receive the original
1999 data in any of three places (checked in this order):

1. the repository root itself  (``web/``)
2. ``web/assets/``            (recommended)
3. the directory above the repository (the classic install layout, ``web/..``)

The first directory that contains every required file wins.  If no raw files
are present but a resource pack exists in ``web/www/``, the pack is used and no
``.res``/``.exe`` file is read.

Usage::

    python tools/assets.py --repo-root .
    python tools/assets.py --repo-root . --json
    python tools/assets.py --repo-root . --assets /path/to/konung --json
    python tools/assets.py --repo-root . --require-raw

Human-readable diagnostics go to stderr so the JSON/printed base on stdout can
be captured by the build scripts.
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

# Every group must match at least one file (case-insensitive).  A group with
# several spellings accepts any of them.
REQUIRED: tuple[tuple[str, ...], ...] = (
    ("konung.res",),
    ("Game.res",),
    ("GRAPH.RES",),
    ("Objects.res",),
    ("CLIENT.EXE", "LAN_KONG.EXE"),
    ("Game.0",),
    ("Game.1",),
    ("Game.2",),
)

# Reported for diagnostics only; missing optional files never fail discovery.
OPTIONAL: tuple[str, ...] = (
    "MENU640R.RES",
    "MENU800R.RES",
    "NEWH640R.RES",
    "NEWH800R.RES",
    "SAVE640R.RES",
    "SAVE800R.RES",
    "INTR640R.RES",
    "INTR800R.RES",
    "QUEST_R.RES",
    "Quest_r.res",
    "SOUNDS.RES",
    "Sounds_.res",
    "FLAMES.RES",
    "LIGHTS.RES",
    "Konung.N",
    "KONUNG.CFG",
    "ListFile.ini",
)

PACK_NAMES: tuple[str, ...] = ("konung.zip.pack", "openkonung.pack.zip")


def _files(directory: Path) -> dict[str, Path]:
    try:
        return {p.name.casefold(): p for p in directory.iterdir() if p.is_file()}
    except OSError:
        return {}


def _dirs(directory: Path) -> dict[str, Path]:
    try:
        return {p.name.casefold(): p for p in directory.iterdir() if p.is_dir()}
    except OSError:
        return {}


def _match(files: dict[str, Path], names: tuple[str, ...]) -> Path | None:
    for name in names:
        found = files.get(name.casefold())
        if found is not None:
            return found
    return None


def inspect(directory: Path) -> dict:
    """Return what is present/missing in one candidate directory."""
    directory = directory.resolve()
    files = _files(directory)
    dirs = _dirs(directory)
    missing: list[str] = []
    found: dict[str, str] = {}
    for group in REQUIRED:
        match = _match(files, group)
        if match is None:
            missing.append("|".join(group))
        else:
            found[group[0]] = match.name
    optional: dict[str, str] = {}
    seen_optional: set[str] = set()
    for name in OPTIONAL:
        key = name.casefold()
        if key in seen_optional:
            continue
        seen_optional.add(key)
        match = _match(files, (name,))
        if match is not None:
            optional[name] = match.name
    for name in ("music", "Video", "originalCD"):
        if name.casefold() in dirs:
            optional[name] = name
    return {
        "dir": str(directory),
        "exists": directory.is_dir(),
        "ok": not missing,
        "missing": missing,
        "found": found,
        "optional": optional,
    }


def candidate_dirs(repo_root: Path) -> list[Path]:
    repo_root = repo_root.resolve()
    return [repo_root, repo_root / "assets", repo_root.parent]


def find_pack(repo_root: Path) -> Path | None:
    www = repo_root.resolve() / "www"
    for name in PACK_NAMES:
        candidate = www / name
        if candidate.is_file():
            return candidate
    return None


def resolve(repo_root: Path, explicit: Path | None = None) -> dict:
    """Resolve the raw install directory and/or the existing pack.

    Returns a dict with ``base`` (str|None), ``pack`` (str|None), ``checked``
    (list of per-directory inspections), and ``ok`` (raw or pack available).
    """
    repo_root = repo_root.resolve()
    checked: list[dict] = []

    if explicit is not None:
        info = inspect(explicit)
        checked.append(info)
        base = info["dir"] if info["ok"] else None
        if not info["ok"]:
            print(
                f"[assets] explicit --assets {info['dir']} is missing: "
                + ", ".join(info["missing"]),
                file=sys.stderr,
            )
    else:
        base = None
        for candidate in candidate_dirs(repo_root):
            info = inspect(candidate)
            checked.append(info)
            if info["ok"]:
                base = info["dir"]
                break

    pack = find_pack(repo_root)
    return {
        "repo_root": str(repo_root),
        "base": base,
        "pack": str(pack) if pack is not None else None,
        "checked": checked,
        "ok": base is not None or pack is not None,
    }


def log_report(result: dict) -> None:
    for info in result["checked"]:
        if info["ok"]:
            print(
                f"[assets] {info['dir']} -> OK "
                f"({len(info['found'])}/{len(REQUIRED)} required files)",
                file=sys.stderr,
            )
        elif not info["exists"]:
            print(f"[assets] {info['dir']} -> not found", file=sys.stderr)
        else:
            missing = ", ".join(info["missing"])
            print(
                f"[assets] {info['dir']} -> missing: {missing}",
                file=sys.stderr,
            )
    if result["base"]:
        print(f"[assets] using raw install: {result['base']}", file=sys.stderr)
    if result["pack"]:
        print(f"[assets] using resource pack: {result['pack']}", file=sys.stderr)
    if not result["ok"]:
        print("", file=sys.stderr)
        print("[assets] ERROR: original Konung files not found.", file=sys.stderr)
        print("[assets] Searched:", file=sys.stderr)
        for i, info in enumerate(result["checked"], 1):
            print(f"  {i}. {info['dir']}", file=sys.stderr)
        print("[assets] Required files:", file=sys.stderr)
        for group in REQUIRED:
            print("  - " + " or ".join(group), file=sys.stderr)
        print("", file=sys.stderr)
        print(
            "[assets] Put the original game files in the repository's assets/ "
            "directory (recommended), the repository root, or keep them in the "
            "directory above the repository.",
            file=sys.stderr,
        )
        print("[assets] See docs/INSTALL.md for the full list.", file=sys.stderr)


def main() -> int:
    here = Path(__file__).resolve().parent
    repo_root_default = here.parent
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--repo-root", type=Path, default=repo_root_default)
    ap.add_argument("--assets", type=Path, default=None, help="explicit install directory")
    ap.add_argument("--json", action="store_true", help="print a JSON result on stdout")
    ap.add_argument("--print-base", action="store_true", help="print the raw install directory")
    ap.add_argument("--print-pack", action="store_true", help="print the resource pack path")
    ap.add_argument(
        "--require-raw",
        action="store_true",
        help="fail when no raw install directory is found (pack alone is not enough)",
    )
    args = ap.parse_args()

    result = resolve(args.repo_root, args.assets)
    log_report(result)

    if args.require_raw and not result["base"]:
        return 2
    if not result["ok"]:
        return 2

    if args.json:
        print(json.dumps(result, indent=2))
    elif args.print_base:
        print(result["base"] or "")
    elif args.print_pack:
        print(result["pack"] or "")
    else:
        print(result["base"] or result["pack"] or "")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
