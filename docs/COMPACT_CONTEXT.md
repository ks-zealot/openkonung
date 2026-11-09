# Konung reverse-engineering — compact handoff context

## Goal
Continue the static RE of the original Konung game from `konung.res`, producing
state-machine/architecture documentation and filing it in MemPalace. Static-only
scope: do not launch the game or create runtime saves.

## Targets / tooling
- Primary target: `konung.res` PE32/i386, base `0x400000`,
  `sha256 d698c4f231489a372cc25212ab2819720a36d7ce5307a045db684135cf679810`.
- `CLIENT.EXE` triaged only (separate earlier/alternate client build).
- `playavi.dll` triaged (one helper thread, frame pump).
- REA provider: Ghidra 12.1.4 via the `rea` MCP tools. The current target is
  `konung.res`; if a later session opens another binary, reopen `konung.res`
  before querying its addresses.
- MemPalace: `C:\Users\z10ba\.mempalace\konung-research`, wing `konung`,
  rooms `overview`, `state-machine`, `startup`, `save-load`, `function-map`,
  `unknowns`, `diary` (plus the bulk docs in `general`).

## Canonical documents (repo root)
- `KONUNG_STATE_MACHINE.md` — states, transitions, frame order, camera,
  projectile pool, world map, action graph, victory/defeat.
- `KONUNG_STARTUP_MAIN.md` — entry/WinMain/WndProc/60 ms loop/PlayAVI thread.
- `KONUNG_SAVE_LOAD.md` — `KONUNG.CFG`, `KONUNG.SAV`, `KONUNG.SA0..9`.
- `KONUNG_FUNCTION_MAP.md` — function inventory + CLIENT triage.
- `KEY_INPUT_SYSTEM.md`, `MOUSE_INPUT_SYSTEM.md` — input pipeline (mode 5 is
  the world map, not inventory/trade).
- `KONUNG_SOURCE_OF_TRUTH.md` — binding contract for geometry/render/data.
- `DIALOG_SYSTEM.md`, `SOUND_SYSTEM.md`, `QUEST_OPCODES.md`.

## Evidence / machine-readable
- `evidence/konung_res_functions.csv` — all 627 procedures with roles.
- `evidence/konung_res_unknown_functions.csv` — remaining unclassified subset.
- `evidence/konung_location_names.csv` — 56 world-map location names.
- `evidence/konung_res_rea_evidence_bundle.json` — REA evidence bundle.

## Current progress
- 627 procedures total: 72 external imports, 43 thunks, 512 internal.
- **309 named game functions**, **203 CRT/library range**,
  **0 unclassified game functions**.
- Every structural/state-machine unknown found earlier is closed:
  `_81D564` camera follow; `_81D6A0`/`0x80B0D0` projectile pool;
  `0x81CF80` discovered map flags; `0x45B394` location names;
  music `_81D60C` / SFX `_81D62C`; save-name `Www Mmm dd hh:mm`;
  `0x03B9 = MM_MCINOTIFY`; quest opcode table `0x46601C` (93 handlers);
  NPC action graph `FUN_004123BC`; PlayAVI helper thread.
- `_81D764` is write-only/dead; `_81D6B8` values 1/3 are unreachable.

## Remaining unclassified addresses
None — all 309 game-range procedures now have roles. The unknown CSV is empty.

## How to continue efficiently
1. Batch-decompile a cluster with `rea_batch_decompile`.
2. Label roles in `C:\Users\z10ba\AppData\Local\Temp\opencode\gen_funcmap.py`
   (the `roles` dict). Existing specific names must win; only add missing ones.
3. Regenerate the CSV (the `list_procedures` JSON is at
   `C:\Users\z10ba\.local\share\opencode\tool-output\tool_15e3ee966001c92fxmgRLX7OOH`):
   `python gen_funcmap.py <list_procedures.json> evidence/konung_res_functions.csv konung.res`
   (use the repo `.venv\Scripts\python.exe`).
4. Regenerate `konung_res_unknown_functions.csv` by filtering
   `role == unclassified`.
5. Update counts in `KONUNG_FUNCTION_MAP.md` / `KONUNG_STATE_MACHINE.md`.
6. Re-file changed docs into MemPalace: delete drawers by their old
   `source_file` (`mempalace_delete_by_source`) then mine a fresh temp directory
   with `mempalace_mine` (wing `konung`). Update the room index drawers.
7. Add a diary entry in AAAK.

## Gotchas
- MemPalace MCP holds the palace lock; use MCP `mempalace_mine`, not the CLI.
- Document files are re-mined from temp staging dirs; old chunks must be
  deleted first or stale text remains searchable.
- Do not use `--release` for `cargo test` in `web/` (`panic = "abort"`).
- All addresses in the docs are `konung.res` VAs.
