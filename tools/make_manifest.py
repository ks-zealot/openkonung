#!/usr/bin/env python3
"""Generate www/manifest.json for the OpenKonung web build.

Two modes:

* ``--base <install-dir>`` scans the original install files and writes the
  per-file manifest used by the packer and by the ``?nopack=1`` development
  fallback.
* ``--from-pack <archive>`` reads the manifest embedded in an existing
  ``konung.zip.pack`` (or reconstructs it from ``pack.json`` for older packs)
  and writes the final manifest without touching any ``.res``/``.exe`` file.

With ``--pack``, the manifest also advertises the single-file resource archive
(``konung.zip.pack``: size + sha256 + pack build id) and folds it into the
content build id.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import zipfile
from pathlib import Path

PACK_MANIFEST = "manifest.raw.json"


def build_id(www: Path, pack: Path | None = None) -> str:
    """Content hash over the shell bundle and (when present) the resource pack,
    so stale JS/WASM/manifest/pack combinations are detectable."""
    h = hashlib.sha256()
    found = False
    for name in ("openkonung.wasm", "app.js", "keymap.js", "index.html"):
        p = www / name
        h.update(name.encode("utf-8"))
        if p.exists():
            h.update(p.read_bytes())
            found = True
        else:
            h.update(b"\x00missing")
    if pack is not None:
        h.update(b"pack")
        if pack.exists():
            h.update(pack.read_bytes())
            found = True
        else:
            h.update(b"\x00missing")
    # Transcoded movies are generated assets too; keep the build id sensitive to
    # them even when the pack is not used.
    video_dir = www / "video"
    if video_dir.is_dir():
        h.update(b"video")
        for p in sorted(video_dir.glob("*.mp4")):
            h.update(p.name.encode("utf-8"))
            h.update(p.read_bytes())
            found = True
    return h.hexdigest()[:12] if found else "dev"


def pack_metadata(pack_path: Path) -> tuple[dict, dict]:
    """Return ``(manifest_pack_info, pack_index)`` for an archive."""
    data = pack_path.read_bytes()
    info = {
        "file": pack_path.name,
        "size": len(data),
        "sha256": hashlib.sha256(data).hexdigest(),
        "format": 3,
    }
    index: dict = {}
    try:
        with zipfile.ZipFile(pack_path) as zf:
            index = json.loads(zf.read("pack.json").decode("utf-8"))
        info["build"] = index.get("build")
        info["entry"] = index.get("entry", "bundle.bin")
    except (KeyError, ValueError, OSError, zipfile.BadZipFile):
        pass
    return info, index


def read_embedded_manifest(pack_path: Path) -> dict | None:
    """Read the raw manifest embedded by the packer, when present."""
    try:
        with zipfile.ZipFile(pack_path) as zf:
            return json.loads(zf.read(PACK_MANIFEST).decode("utf-8"))
    except (KeyError, ValueError, OSError, zipfile.BadZipFile):
        return None


def manifest_from_pack_index(index: dict, www: Path) -> dict:
    """Reconstruct a raw manifest from ``pack.json`` (legacy packs)."""
    by_kind = {
        int(item.get("kind", -1)): item.get("name")
        for item in index.get("resources", [])
        if item.get("name")
    }

    menu: dict = {}
    for width, base_kind in (("640", 5), ("800", 6)):
        entry = {
            "menu": by_kind.get(base_kind),
            "newh": by_kind.get(base_kind + 2),
            "save": by_kind.get(base_kind + 4),
        }
        if entry["menu"] and entry["newh"]:
            menu[width] = entry

    intr = {}
    for width, kind in (("640", 11), ("800", 12)):
        if kind in by_kind:
            intr[width] = by_kind[kind]

    videos: dict = {}
    for item in index.get("videos", []):
        name = item.get("name")
        if not name:
            continue
        rel = f"video/{name}.mp4"
        path = www / rel
        videos[name.lower()] = {
            "file": rel,
            "size": path.stat().st_size if path.is_file() else int(item.get("bytes", 0)),
            "sha256": hashlib.sha256(path.read_bytes()).hexdigest() if path.is_file() else "",
        }

    return {
        "base": "/",
        "resources": {
            "game": by_kind.get(0),
            "graph": by_kind.get(1),
            "objects": by_kind.get(2),
            "exe": by_kind.get(3),
            "names": by_kind.get(4),
        },
        "menu": menu,
        "intr": intr,
        "quest": by_kind.get(13),
        "sounds": by_kind.get(14),
        "flames": by_kind.get(15),
        "lights": by_kind.get(16),
        "light_maps": [
            {"id": int(item["id"]), "file": item["name"]}
            for item in index.get("lights", [])
            if "id" in item and "name" in item
        ],
        "videos": videos,
        "profiles": [
            item["name"] for item in index.get("profiles", []) if "name" in item
        ],
        "konungs": [
            {"id": int(item["id"]), "file": item["name"]}
            for item in index.get("konungs", [])
            if "id" in item and "name" in item
        ],
        "music": [item["name"] for item in index.get("music", []) if "name" in item],
    }


def scan_install(base: Path) -> dict:
    """Build the raw manifest by scanning an original install directory."""
    entries = sorted(base.iterdir()) if base.is_dir() else []

    def actual_name(name: str) -> str | None:
        """Return the on-disk spelling of ``name`` (case-insensitive match).

        Windows resolves file names case-insensitively; writing the requested
        candidate spelling into the manifest would then be wrong on the
        case-sensitive Linux image (e.g. ``QUEST_R.RES`` vs ``Quest_r.res``).
        """
        target = name.casefold()
        for p in entries:
            if p.is_file() and p.name.casefold() == target:
                return p.name
        return None

    def require(name: str) -> str:
        found = actual_name(name)
        if found is None:
            raise SystemExit(f"error: required original file not found: {name}")
        return found

    exe = actual_name("CLIENT.EXE") or actual_name("LAN_KONG.EXE")
    if exe is None:
        raise SystemExit("error: neither CLIENT.EXE nor LAN_KONG.EXE found")

    names = require("konung.res")

    konungs = []
    for p in entries:
        if not p.is_file():
            continue
        m = re.match(r"^konung\.(\d+)$", p.name, re.I)
        if m:
            konungs.append({"id": int(m.group(1)), "file": p.name})
    konungs.sort(key=lambda x: x["id"])

    def pick(name: str) -> str | None:
        return actual_name(name)

    # Background music: the original install's `music/` folder. Tracks are Ogg
    # Vorbis (`trackNN.ogg`); `.wav` is still accepted, but when both formats
    # exist for the same stem the Ogg file wins. Paths are relative to `base`
    # ("/"), so the browser fetches /music/....
    music_by_stem: dict[str, Path] = {}
    music_dir = base / "music"
    if music_dir.is_dir():
        for p in music_dir.iterdir():
            if not p.is_file() or p.suffix.lower() not in (".ogg", ".wav"):
                continue
            prev = music_by_stem.get(p.stem)
            if prev is None or (prev.suffix.lower() == ".wav" and p.suffix.lower() == ".ogg"):
                music_by_stem[p.stem] = p
    music = [
        f"music/{music_by_stem[stem].name}"
        for stem in sorted(music_by_stem, key=str.lower)
    ]

    menu = {}
    for width in (640, 800):
        entry = {
            "menu": pick(f"MENU{width}R.RES"),
            "newh": pick(f"NEWH{width}R.RES"),
            "save": pick(f"SAVE{width}R.RES"),
        }
        if entry["menu"] and entry["newh"]:
            menu[str(width)] = entry

    # Original in-game interface image archives (INTR{width}R.RES), per
    # resolution; decoded by the Rust INTR loader at game start.
    intr = {}
    for width in (640, 800):
        name = pick(f"INTR{width}R.RES")
        if name:
            intr[str(width)] = name

    # Dialogue/quest database (the executable opens the literal name
    # `QUEST_R.RES`; the install ships it as a case/underscore spelling).
    # NOTE: `QUESTR.RES` and `Quest_r.res` are NOT the same file. `QUESTR.RES`
    # is a placeholder database (its dialogue 97/96 are literal "???" stubs);
    # `Quest_r.res` is the real shipped dialogue script. Prefer the real one.
    quest = None
    for candidate in ("QUEST_R.RES", "Quest_r.res", "QUESTR.RES"):
        found = actual_name(candidate)
        if found is not None:
            quest = found
            break

    # Sound archive (`SOUNDS.RES`; the file may be shipped lowercased/underscored).
    sounds = None
    for candidate in ("SOUNDS.RES", "Sounds_.res", "SOUNDS_.RES"):
        found = actual_name(candidate)
        if found is not None:
            sounds = found
            break

    # Prop fire animation archive (`FLAMES.RES`, 74 frames).
    flames = pick("FLAMES.RES")

    # Lighting archive + per-map LIGHTS.N light-index maps.
    lights_res = pick("LIGHTS.RES")
    light_maps = []
    for p in entries:
        if not p.is_file():
            continue
        m = re.match(r"^LIGHTS\.(\d+)$", p.name, re.I)
        if m:
            light_maps.append({"id": int(m.group(1)), "file": p.name})
    light_maps.sort(key=lambda x: x["id"])

    # Browser-playable movies transformed from the original AVI/IV50 `.flm`
    # files by tools/transcode_videos.py. Keys are lowercase clip stems.
    videos = {}
    video_dir = Path(__file__).resolve().parent.parent / "www" / "video"
    if video_dir.is_dir():
        for p in sorted(video_dir.glob("*.mp4")):
            rel = f"video/{p.name}"
            videos[p.stem.lower()] = {
                "file": rel,
                "size": p.stat().st_size,
                "sha256": hashlib.sha256(p.read_bytes()).hexdigest(),
            }

    profiles = [require("Game.0"), require("Game.1"), require("Game.2")]

    return {
        "base": "/",
        "resources": {
            "game": require("Game.res"),
            "graph": require("GRAPH.RES"),
            "objects": require("Objects.res"),
            "exe": exe,
            "names": names,
        },
        "menu": menu,
        "intr": intr,
        "quest": quest,
        "sounds": sounds,
        "flames": flames,
        "lights": lights_res,
        "light_maps": light_maps,
        "videos": videos,
        "profiles": profiles,
        "konungs": konungs,
        "music": music,
    }


def main() -> None:
    ap = argparse.ArgumentParser(description="Generate the OpenKonung web manifest")
    ap.add_argument("--base", type=Path, default=None, help="original install directory")
    ap.add_argument("--out", type=Path, required=True, help="output manifest.json path")
    ap.add_argument(
        "--pack",
        type=Path,
        default=None,
        help="resource pack archive to advertise (generated by examples/pack.rs)",
    )
    ap.add_argument(
        "--from-pack",
        type=Path,
        default=None,
        help="read the manifest from an existing pack instead of scanning --base",
    )
    args = ap.parse_args()

    if args.from_pack is None and args.base is None:
        raise SystemExit("error: pass --base <install-dir> or --from-pack <archive>")

    www = args.out.resolve().parent
    pack_path = args.pack.resolve() if args.pack is not None else None
    if pack_path is not None and not pack_path.is_file():
        raise SystemExit(f"error: resource pack not found: {pack_path}")

    if args.from_pack is not None:
        source_pack = args.from_pack.resolve()
        if not source_pack.is_file():
            raise SystemExit(f"error: resource pack not found: {source_pack}")
        manifest = read_embedded_manifest(source_pack)
        if manifest is None:
            _, index = pack_metadata(source_pack)
            manifest = manifest_from_pack_index(index, www)
            print(f"manifest: reconstructed from {source_pack.name}/pack.json")
        else:
            print(f"manifest: read embedded {PACK_MANIFEST} from {source_pack.name}")
        # If the caller did not pass --pack, advertise the pack we read.
        if pack_path is None:
            pack_path = source_pack
    else:
        base = args.base.resolve()
        manifest = scan_install(base)

    manifest["build"] = build_id(www, pack_path)
    if pack_path is not None:
        info, _ = pack_metadata(pack_path)
        manifest["pack"] = info

    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    suffix = f", pack={info['file']} ({info['size']} B)" if pack_path is not None else ""
    print(
        f"manifest: {len(manifest.get('konungs', []))} Konung maps, "
        f"{len(manifest.get('light_maps', []))} light maps, "
        f"{len(manifest.get('videos', {}))} videos, "
        f"exe={manifest.get('resources', {}).get('exe')}{suffix} -> {args.out}"
    )


if __name__ == "__main__":
    main()
