#!/usr/bin/env python3
"""Dump the project MemPalace to plain files.

Reads the ChromaDB SQLite database directly (no extra Python dependency) and
writes:

* ``docs/MEMPALACE_DUMP.md``      human-readable, grouped by wing/room
* ``docs/mempalace_dump.jsonl``   one logical drawer per line
* ``docs/MEMPALACE_COVERAGE.md``  which drawers are backed by a repo file

Usage::

    python tools/dump_mempalace.py
    python tools/dump_mempalace.py --palace ~/.mempalace/konung-research
    python tools/dump_mempalace.py --palace /path/chroma.sqlite3 --out-dir docs
"""
from __future__ import annotations

import argparse
import json
import sqlite3
from collections import Counter, defaultdict
from datetime import datetime, timezone
from pathlib import Path

DEFAULT_PALACE = Path.home() / ".mempalace" / "konung-research"
TEXT_KEYS = (
    "wing",
    "room",
    "source_file",
    "topic",
    "filed_at",
    "date",
    "agent",
    "added_by",
    "hall",
    "type",
    "parent_drawer_id",
    "parent_entry_id",
    "chunk_index",
    "chunk_total",
)


def sqlite_path(palace: Path) -> Path:
    palace = palace.expanduser()
    if palace.is_file():
        return palace
    candidate = palace / "chroma.sqlite3"
    if candidate.is_file():
        return candidate
    raise SystemExit(f"error: no chroma.sqlite3 under {palace}")


def read_rows(path: Path) -> list[dict]:
    con = sqlite3.connect(f"file:{path}?mode=ro", uri=True)
    try:
        cur = con.execute(
            """
            SELECT e.embedding_id, e.created_at, m.key,
                   m.string_value, m.int_value, m.float_value, m.bool_value
            FROM embeddings AS e
            JOIN embedding_metadata AS m ON m.id = e.id
            """
        )
        rows: dict[str, dict] = {}
        for embedding_id, created_at, key, s, i, f, b in cur:
            row = rows.setdefault(
                embedding_id,
                {"embedding_id": embedding_id, "created_at": created_at, "meta": {}},
            )
            value = s if s is not None else (i if i is not None else (f if f is not None else b))
            row["meta"][key] = value
        return list(rows.values())
    finally:
        con.close()


def logical_drawers(rows: list[dict]) -> list[dict]:
    groups: dict[str, list[dict]] = defaultdict(list)
    for row in rows:
        meta = row["meta"]
        key = meta.get("parent_drawer_id") or row["embedding_id"]
        groups[key].append(row)

    drawers: list[dict] = []
    for drawer_id, chunks in groups.items():
        chunks.sort(key=lambda r: int(r["meta"].get("chunk_index") or 0))
        meta: dict = {}
        content_parts: list[str] = []
        for chunk in chunks:
            for key, value in chunk["meta"].items():
                if key == "chroma:document":
                    continue
                meta.setdefault(key, value)
            document = chunk["meta"].get("chroma:document")
            if document:
                content_parts.append(str(document))
        drawers.append(
            {
                "drawer_id": drawer_id,
                "wing": str(meta.get("wing") or "unknown"),
                "room": str(meta.get("room") or "general"),
                "metadata": {k: v for k, v in meta.items() if k != "chroma:document"},
                "content": "\n".join(content_parts).strip(),
                "chunks": len(chunks),
            }
        )
    drawers.sort(key=lambda d: (d["wing"], d["room"], str(d["metadata"].get("filed_at") or ""), d["drawer_id"]))
    return drawers


def repo_files(repo_root: Path) -> dict[str, list[str]]:
    """basename -> list of repo-relative paths (skip build/VCS directories)."""
    skip = {".git", "target", "node_modules", ".venv", "__pycache__", ".idea"}
    found: dict[str, list[str]] = defaultdict(list)
    for path in repo_root.rglob("*"):
        if not path.is_file():
            continue
        if any(part in skip for part in path.parts):
            continue
        found[path.name.casefold()].append(str(path.relative_to(repo_root)))
    return found


def coverage(drawers: list[dict], repo_root: Path) -> dict:
    files = repo_files(repo_root)
    md_files = {name for name in files if name.endswith(".md")}
    categories: Counter = Counter()
    rows: list[dict] = []
    for drawer in drawers:
        source = drawer["metadata"].get("source_file")
        if not source:
            category = "no source"
        else:
            base = Path(str(source)).name.casefold()
            if base in md_files:
                category = "markdown"
            elif base in files:
                category = "other repo file"
            else:
                category = "missing"
        categories[category] += 1
        rows.append(
            {
                "drawer_id": drawer["drawer_id"],
                "wing": drawer["wing"],
                "room": drawer["room"],
                "source_file": source or "",
                "category": category,
            }
        )
    return {"categories": categories, "rows": rows, "files": files}


def write_markdown(path: Path, drawers: list[dict], source: Path) -> None:
    now = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    by_wing: dict[str, dict[str, list[dict]]] = defaultdict(lambda: defaultdict(list))
    for drawer in drawers:
        by_wing[drawer["wing"]][drawer["room"]].append(drawer)

    lines = [
        "# MemPalace dump",
        "",
        f"- Source: `{source}`",
        f"- Generated: {now}",
        f"- Logical drawers: **{len(drawers)}**",
        "",
        "This file is a plain-text backup of the project memory. It is generated by",
        "`tools/dump_mempalace.py`; do not hand-edit.",
        "",
        "Historical `web/...` paths in drawer content refer to the original install",
        "layout; in this repository the root is `web/`.",
        "",
    ]
    for wing in sorted(by_wing):
        lines.append(f"## Wing: {wing}")
        lines.append("")
        for room in sorted(by_wing[wing]):
            items = by_wing[wing][room]
            lines.append(f"### Room: {room} ({len(items)} drawers)")
            lines.append("")
            for drawer in items:
                meta = drawer["metadata"]
                lines.append(f"#### `{drawer['drawer_id']}`")
                lines.append("")
                for key in TEXT_KEYS:
                    if key in meta and meta[key] not in (None, ""):
                        lines.append(f"- **{key}:** {meta[key]}")
                if drawer["chunks"] > 1:
                    lines.append(f"- **chunks:** {drawer['chunks']}")
                lines.append("")
                lines.append(drawer["content"] or "_(empty)_")
                lines.append("")
    path.write_text("\n".join(lines), encoding="utf-8")


def write_jsonl(path: Path, drawers: list[dict]) -> None:
    with path.open("w", encoding="utf-8") as f:
        for drawer in drawers:
            f.write(json.dumps(drawer, ensure_ascii=False) + "\n")


def write_coverage(path: Path, result: dict, total: int) -> None:
    categories: Counter = result["categories"]
    rows: list[dict] = result["rows"]
    lines = [
        "# MemPalace coverage",
        "",
        f"Total logical drawers: **{total}**",
        "",
        "| Category | Drawers | Meaning |",
        "|---|---:|---|",
        f"| markdown | {categories.get('markdown', 0)} | `source_file` names a markdown file in this repo. |",
        f"| other repo file | {categories.get('other repo file', 0)} | `source_file` names a non-markdown file in this repo (CSV/source). |",
        f"| missing | {categories.get('missing', 0)} | `source_file` names a file that is not in this repo. |",
        f"| no source | {categories.get('no source', 0)} | No `source_file` (diary, package notes, session memory). |",
        "",
        "The full content of every drawer, including the `missing` and `no source`",
        "categories, is preserved in `MEMPALACE_DUMP.md` and `mempalace_dump.jsonl`.",
        "",
        "## Drawers not backed by a repository markdown file",
        "",
        "| Drawer | Wing / room | Source | Category |",
        "|---|---|---|---|",
    ]
    for row in rows:
        if row["category"] == "markdown":
            continue
        lines.append(
            f"| `{row['drawer_id']}` | {row['wing']} / {row['room']} | "
            f"`{row['source_file'] or '(none)'}` | {row['category']} |"
        )
    lines.append("")
    lines.append("## Missing source files")
    lines.append("")
    missing = sorted({row["source_file"] for row in rows if row["category"] == "missing"})
    if missing:
        for source in missing:
            lines.append(f"- `{source}`")
    else:
        lines.append("_None._")
    lines.append("")
    path.write_text("\n".join(lines), encoding="utf-8")


def main() -> int:
    here = Path(__file__).resolve().parent
    repo_root = here.parent
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--palace", type=Path, default=DEFAULT_PALACE)
    ap.add_argument("--out-dir", type=Path, default=repo_root / "docs")
    args = ap.parse_args()

    source = sqlite_path(args.palace)
    rows = read_rows(source)
    drawers = logical_drawers(rows)
    out = args.out_dir.resolve()
    out.mkdir(parents=True, exist_ok=True)

    write_markdown(out / "MEMPALACE_DUMP.md", drawers, source)
    write_jsonl(out / "mempalace_dump.jsonl", drawers)
    result = coverage(drawers, repo_root)
    write_coverage(out / "MEMPALACE_COVERAGE.md", result, len(drawers))

    print(
        f"dump: {len(drawers)} logical drawers from {source}\n"
        f"  {out / 'MEMPALACE_DUMP.md'}\n"
        f"  {out / 'mempalace_dump.jsonl'}\n"
        f"  {out / 'MEMPALACE_COVERAGE.md'}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
