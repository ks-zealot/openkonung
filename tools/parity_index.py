#!/usr/bin/env python3
"""Function-level parity ledger for `konung.res` (game code range).

The remake cites original virtual addresses in doc comments, but no artefact
records *whether each ported function was ever compared against the original
body*. This tool builds and validates that ledger.

Sources
-------
* `evidence/konung_res_functions.csv` - REA/Ghidra inventory: one row per
  procedure (`address,name,kind,region,role,evidence_id`). The `role` strings
  are auto-derived and several are wrong (see FUNCTION_PARITY.md S1.2), so the
  ledger stores them verbatim and never treats them as evidence.
* `web/src/**/*.rs` - `0x4xxxxx` / `FUN_004xxxxx` citations -> VA index.

Output / storage
----------------
* `evidence/function_parity.csv` - the ledger, one row per game-range VA:
  `va,csv_role,system,rust_symbols,verdict,fix_status,evidence_id,divergence,test`
  Verdicts are `PARITY`, `PARITY-DEVIATION`, `GAP-BRANCH`, `GAP-VALUE`,
  `GAP-ORDER`, `GAP-STATE`, `GAP-MISSING`, `N/A-PLATFORM` (empty = not audited).
  Regenerating preserves every non-empty cell of an existing row; only
  `rust_symbols` is refreshed from the source scan.

Usage
-----
    python tools/parity_index.py            # write/refresh the ledger
    python tools/parity_index.py --check    # validate, print summary, exit 1 on holes
    python tools/parity_index.py --system AI

`--check` fails when a game-range VA is missing from the ledger, a verdict is
unrecognised, an audited row has no evidence id, or a cited Rust path no longer
exists.
"""

from __future__ import annotations

import argparse
import csv
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]  # .../original_install
WEB = REPO / "web"
SRC = WEB / "src"
INVENTORY = REPO / "evidence" / "konung_res_functions.csv"
LEDGER = REPO / "evidence" / "function_parity.csv"

FIELDS = [
    "va",
    "csv_role",
    "system",
    "rust_symbols",
    "verdict",
    "fix_status",
    "evidence_id",
    "divergence",
    "test",
]

VERDICTS = {
    "PARITY",
    "PARITY-DEVIATION",
    "GAP-BRANCH",
    "GAP-VALUE",
    "GAP-ORDER",
    "GAP-STATE",
    "GAP-MISSING",
    "N/A-PLATFORM",
}
FIX_STATUS = {"", "open", "approved", "fixed", "wontfix", "n/a"}

# Explicit system assignment for the audited and scheduled game functions.
# Anything not listed falls back to `classify()`'s role-keyword heuristic and
# is reported as provisional until a wave audits it.
SYSTEM_BY_VA = {
    # --- Wave A: party/companion chain -------------------------------------
    "0x421B50": "AI",
    "0x41ECC8": "AI",
    "0x41E2D4": "AI",
    "0x41EB48": "AI",
    "0x41F3C8": "AI",
    "0x4101E4": "AI",
    "0x410E84": "AI",
    "0x41130C": "AI",
    "0x41EFD8": "AI",
    "0x41F048": "AI",
    # --- Wave A: object/enemy AI -------------------------------------------
    "0x412C30": "AI",
    "0x412E00": "AI",
    "0x4123BC": "AI",
    "0x415FC8": "AI",
    "0x4168A4": "AI",
    "0x415C40": "AI",
    "0x413A84": "AI",
    "0x4170E0": "AI",
    "0x417190": "AI",
    "0x4116CC": "AI",
    "0x41FB64": "AI",
    "0x41FF68": "AI",
    "0x420098": "AI",
    "0x420190": "AI",
    "0x410FFC": "AI",
    "0x411FD0": "AI",
    "0x421964": "AI",
    "0x410450": "AI",
    "0x410728": "AI",
    "0x414A34": "AI",
    "0x413DE4": "AI",
    # --- Wave A: damage / combat math --------------------------------------
    "0x41AB7C": "COMBAT",
    "0x41AC40": "COMBAT",
    "0x41AD44": "COMBAT",
    "0x41AEA8": "COMBAT",
    "0x41B060": "COMBAT",
    "0x41B0C4": "COMBAT",
    "0x41B1A4": "COMBAT",
    "0x41B2A8": "COMBAT",
    "0x41B42C": "COMBAT",
    "0x41C1E8": "COMBAT",
    "0x41C3CC": "COMBAT",
    "0x41C69C": "COMBAT",
    "0x41BF44": "COMBAT",
    "0x420F30": "COMBAT",
    "0x41B658": "COMBAT",
    "0x418480": "COMBAT",
    # --- Wave A: movement / pathfinding ------------------------------------
    "0x4142D4": "MOVEMENT",
    "0x41435C": "MOVEMENT",
    "0x414ABC": "MOVEMENT",
    "0x43DA1F": "MOVEMENT",
    "0x43D922": "MOVEMENT",
    "0x41165C": "MOVEMENT",
    "0x438640": "MOVEMENT",
    "0x43881C": "MOVEMENT",
    "0x43897C": "MOVEMENT",
    "0x438A78": "MOVEMENT",
    "0x438AB4": "MOVEMENT",
    "0x438ED8": "MOVEMENT",
    "0x414078": "MOVEMENT",
    "0x438090": "MOVEMENT",
    # --- Wave B: trading ---------------------------------------------------
    "0x431E2C": "TRADE",
    "0x430D6C": "TRADE",
    "0x4203F4": "TRADE",
    "0x41AE00": "TRADE",
    "0x41B5A8": "TRADE",
    "0x41B7E4": "TRADE",
    "0x42462C": "TRADE",
    "0x421AD0": "TRADE",
    "0x4245B4": "TRADE",
    # `0x41079C` is the XP grant + AUTO auto-spend engine, not money.
    "0x41079C": "COMBAT",
    # --- Wave C: settlement economics --------------------------------------
    "0x41D048": "SETTLEMENT",
    "0x416670": "SETTLEMENT",
    "0x4172AC": "SETTLEMENT",
    "0x4175FC": "SETTLEMENT",
    "0x4179C8": "SETTLEMENT",
    "0x417F84": "SETTLEMENT",
    "0x41182C": "SETTLEMENT",
    "0x411078": "SETTLEMENT",
    "0x4110EC": "SETTLEMENT",
    "0x41093C": "SETTLEMENT",
    "0x418E6C": "SETTLEMENT",
    "0x4313B0": "SETTLEMENT",
    "0x431520": "SETTLEMENT",
    "0x420A8C": "SETTLEMENT",
    "0x41BD3C": "SETTLEMENT",
    "0x417144": "SETTLEMENT",
    # --- Wave D: dialogue / quest ------------------------------------------
    "0x43159C": "QUEST",
    "0x434BB8": "QUEST",
    "0x434C74": "QUEST",
    "0x434FE8": "QUEST",
    "0x435188": "QUEST",
    "0x435290": "QUEST",
    "0x43395C": "QUEST",
    "0x433A8C": "QUEST",
    "0x4347D0": "QUEST",
    "0x431820": "QUEST",
    "0x432954": "QUEST",
    "0x431DCC": "QUEST",
    "0x431654": "QUEST",
    "0x432F8C": "QUEST",
    "0x433FF8": "QUEST",
}

CITE_RE = re.compile(r"(?i)(?:0x|FUN_00)(4[0-9a-f]{5})")

# Case-insensitive VA lookup (the inventory stores lowercase hex).
SYSTEM_BY_VA = {k.lower(): v for k, v in SYSTEM_BY_VA.items()}


def read_inventory() -> list[dict[str, str]]:
    with INVENTORY.open(newline="", encoding="utf-8") as fh:
        rows = [r for r in csv.DictReader(fh) if r.get("region") == "game"]
    if not rows:
        sys.exit(f"no game-range rows in {INVENTORY}")
    return rows


def scan_citations() -> dict[str, list[str]]:
    hits: dict[str, list[str]] = {}
    for path in sorted(SRC.rglob("*.rs")):
        text = path.read_text(encoding="utf-8", errors="replace")
        rel = path.relative_to(SRC).as_posix()
        for m in CITE_RE.finditer(text):
            va = "0x" + m.group(1).upper()
            bucket = hits.setdefault(va, [])
            if rel not in bucket:
                bucket.append(rel)
    return hits


def classify(va: str, role: str) -> str:
    curated = SYSTEM_BY_VA.get(va.lower())
    if curated:
        return curated
    r = role.lower()
    if "quest opcode" in r or "dialogue" in r or "script" in r:
        return "QUEST"
    if "settlement" in r or "restock" in r or "mentor" in r or "population" in r:
        return "SETTLEMENT"
    if "trade" in r or "price" in r or "money" in r or "inventory" in r:
        return "TRADE"
    if any(k in r for k in ("ai", "actor", "group", "target", "attack", "route",
                            "move", "walk", "step", "chase", "retreat")):
        return "AI"
    if "damage" in r or "armor" in r or "hit" in r or "dodge" in r or "stat" in r:
        return "COMBAT"
    if "sound" in r or "music" in r or "mci" in r or "directsound" in r:
        return "AUDIO"
    if "directdraw" in r or "blit" in r or "draw" in r or "render" in r or "sprite" in r:
        return "RENDER"
    if "load" in r or "save" in r or "init" in r or "startup" in r or "crt" in r:
        return "LOADER"
    if "key" in r or "mouse" in r or "cursor" in r or "window" in r:
        return "INPUT"
    return "MISC"


def read_ledger() -> dict[str, dict[str, str]]:
    if not LEDGER.exists():
        return {}
    with LEDGER.open(newline="", encoding="utf-8") as fh:
        return {r["va"]: r for r in csv.DictReader(fh)}


def build() -> list[dict[str, str]]:
    inventory = read_inventory()
    cites = scan_citations()
    prior = read_ledger()
    out: list[dict[str, str]] = []
    for row in sorted(inventory, key=lambda r: int(r["address"], 16)):
        va = row["address"]
        old = prior.get(va, {})
        entry = {
            "va": va,
            "csv_role": row["role"],
            "system": classify(va, row["role"])
            if va.lower() in SYSTEM_BY_VA
            else (old.get("system") or classify(va, row["role"])),
            "rust_symbols": " ".join(cites.get(va, [])),
            "verdict": old.get("verdict", ""),
            "fix_status": old.get("fix_status", ""),
            "evidence_id": old.get("evidence_id", ""),
            "divergence": old.get("divergence", ""),
            "test": old.get("test", ""),
        }
        out.append(entry)
    return out


def write(rows: list[dict[str, str]]) -> None:
    LEDGER.parent.mkdir(parents=True, exist_ok=True)
    with LEDGER.open("w", newline="", encoding="utf-8") as fh:
        w = csv.DictWriter(fh, fieldnames=FIELDS)
        w.writeheader()
        w.writerows(rows)


def check(rows: list[dict[str, str]], system: str | None) -> int:
    problems: list[str] = []
    seen = set()
    counts: dict[str, int] = {}
    audited = 0
    for row in rows:
        va = row["va"]
        if va in seen:
            problems.append(f"{va}: duplicate row")
        seen.add(va)
        verdict = row["verdict"].strip()
        if verdict and verdict not in VERDICTS:
            problems.append(f"{va}: unknown verdict {verdict!r}")
        if verdict:
            audited += 1
        if verdict and not row["evidence_id"].strip() and verdict != "N/A-PLATFORM":
            problems.append(f"{va}: audited without evidence_id")
        status = row["fix_status"].strip()
        if status and status not in FIX_STATUS:
            problems.append(f"{va}: unknown fix_status {status!r}")
        if status in {"approved", "fixed"} and not verdict.startswith("GAP"):
            problems.append(f"{va}: fix_status {status} on non-gap verdict")
        for rel in row["rust_symbols"].split():
            if not (SRC / rel).exists():
                problems.append(f"{va}: cited Rust file missing: {rel}")
        counts[row["system"]] = counts.get(row["system"], 0) + 1

    total = len(rows)
    print(f"ledger rows: {total}  audited: {audited}  unaudited: {total - audited}")
    print(f"{'system':<12}{'functions':>10}{'audited':>9}")
    for name in sorted(counts):
        sub = [r for r in rows if r["system"] == name]
        done = sum(1 for r in sub if r["verdict"].strip())
        print(f"{name:<12}{len(sub):>10}{done:>9}")
    if system:
        sub = [r for r in rows if r["system"] == system]
        print(f"\n{system}:")
        for r in sub:
            print(f"  {r['va']}  {r['verdict'] or '-':<16} {r['fix_status'] or '-':<9}"
                  f" {r['csv_role']}")
    if problems:
        print(f"\n{len(problems)} problem(s):")
        for p in problems:
            print("  " + p)
        return 1
    print("\nledger OK")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--check", action="store_true", help="validate without writing")
    ap.add_argument("--system", help="list one system in the summary")
    args = ap.parse_args()
    rows = build()
    if not args.check:
        write(rows)
        print(f"wrote {LEDGER.relative_to(REPO)} ({len(rows)} rows)")
    return check(rows, args.system)


if __name__ == "__main__":
    raise SystemExit(main())
