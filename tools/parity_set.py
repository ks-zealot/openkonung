#!/usr/bin/env python3
"""Set audited fields in `evidence/function_parity.csv`.

Used by the parity audit waves so ledger rows are written reproducibly instead
of hand-edited. Reads a JSON file: a list of objects with `va` plus any of
`verdict`, `fix_status`, `evidence_id`, `divergence`, `test`; `system` may also
be set. Unknown VAs are an error (typos must not create phantom rows).

    python tools/parity_set.py findings.json
"""

from __future__ import annotations

import csv
import json
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
LEDGER = REPO / "evidence" / "function_parity.csv"
FIELDS = ["va", "csv_role", "system", "rust_symbols", "verdict", "fix_status",
          "evidence_id", "divergence", "test"]
SETTABLE = {"system", "verdict", "fix_status", "evidence_id", "divergence", "test"}


def main(argv: list[str]) -> int:
    if len(argv) != 1:
        return int(bool(sys.stderr.write(__doc__)))
    payload = json.loads(Path(argv[0]).read_text(encoding="utf-8"))
    if not isinstance(payload, list):
        sys.exit("expected a JSON list of {va, ...} objects")
    with LEDGER.open(newline="", encoding="utf-8") as fh:
        rows = list(csv.DictReader(fh))
    # The inventory stores lowercase hex; accept either case from findings.
    index = {r["va"].lower(): r for r in rows}
    for item in payload:
        va = item.get("va", "")
        row = index.get(va.lower())
        if row is None:
            sys.exit(f"unknown VA {va!r} (not a game-range row)")
        for key, value in item.items():
            if key == "va":
                continue
            if key not in SETTABLE:
                sys.exit(f"{va}: field {key!r} is not settable")
            row[key] = str(value)
    with LEDGER.open("w", newline="", encoding="utf-8") as fh:
        w = csv.DictWriter(fh, fieldnames=FIELDS)
        w.writeheader()
        w.writerows(rows)
    print(f"updated {len(payload)} row(s)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
