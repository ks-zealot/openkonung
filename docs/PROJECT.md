# OpenKonung — Project Documentation (current state)

> Scope: this document describes **what the project is and how it is built,
> run, tested and deployed today**. Correctness rules and reverse-engineering
> evidence live in [`KONUNG_SOURCE_OF_TRUTH.md`](KONUNG_SOURCE_OF_TRUTH.md);
> build/agent conventions live in [`AGENTS.md`](AGENTS.md).
> Per-file responsibilities are in [`MODULES.md`](MODULES.md).

## 1. What this is

A reverse-engineered, browser playable remake of the 1999 game **Konung**. The
remake reads the original shipped data files directly and reimplements the
behavior of the original `konung.res` (PE32/i386) executable.

Two implementations live side by side:

- `src/*.rs` — the canonical **Rust → WebAssembly** browser port.
  **Active development.**
- The legacy Python decoders/viewers in the original install directory are kept
  as **reference oracles** (they still decode the original files and back the
  parity check).

`KONUNG_SOURCE_OF_TRUTH.md` is the binding implementation contract: evidence
levels, the coordinate/render pipeline, exact file layouts, per-subsystem rules
and remaining unknowns. When code and the contract disagree, the contract wins
and the code is re-checked against the original binaries.

## 2. Repository layout

```text
web/                                   # git repository root
  README.md                            # install / build / run / MemPalace
  assets/                              # original game files (recommended; gitignored)
  docs/                                # all other documentation
    PROJECT.md  MODULES.md  AGENTS.md
    KONUNG_SOURCE_OF_TRUTH.md          # canonical R/E contract
    *_SYSTEM.md  CLIENT_EXE_*.md  ...
  src/                                 # Rust source (see MODULES.md)
  www/    app.js index.html            # browser shell (hand-written)
          openkonung.wasm              # GENERATED build artifact
          konung.zip.pack              # GENERATED resource pack
          manifest.json                # GENERATED build artifact
  tools/                               # smoke tests, PNG exporters, parity
  examples/dump.rs                     # Rust↔Python parity dump
  Cargo.toml  build.ps1  build.sh
  serve.ps1   serve.sh   server.py
```

Treat the original data files and `target/` as immutable inputs/artifacts.
`www/openkonung.wasm`, `www/konung.zip.pack`, `www/manifest.json` and
`www/manifest.raw.json` are generated — never hand-edit.

## 3. Build / run / test (run from the repository root)

| Task | Command |
|---|---|
| Build wasm + regenerate manifest | `.\build.ps1` (Windows) / `bash build.sh` |
| Typecheck all targets | `cargo check --all-targets` |
| Unit tests | `cargo test` (**dev profile**; do **not** use `--release` — `panic = "abort"`) |
| Headless wasm smoke | `node tools/smoke.mjs` |
| Frontend smoke (menu → New Game → New Hero → confirm → game) | `node tools/frontend_smoke.mjs` |
| Page smoke over HTTP | start `serve.ps1`/`serve.sh`, then `node tools/page_smoke.mjs [url]` |
| Run in a browser | `.\serve.ps1` or `bash serve.sh` → http://localhost:8000/ |
| Rust↔Python parity | `python tools/parity.py [base] [profile] [variant]` vs `cargo run --release --example dump -- <base> [profile] [variant]` |
| Render frames to PNG | `node tools/frame_png.mjs` (also `menu_png.mjs`, `intr_png.mjs`, `intr_sheet.mjs`, `character_png.mjs`) |

`build.ps1`/`build.sh` require the `wasm32-unknown-unknown` target. The first
run reads the original files from `assets/`, the repository root, or the parent
directory, copies `target/…/openkonung.wasm` into `www/`, and writes
`www/konung.zip.pack` + `www/manifest.json` via `tools/make_manifest.py` and
`examples/pack.rs`. Later runs reuse the pack and never read `.res`/`.exe`
(pass `-Repack`/`--repack` after changing the original assets).

**Current verified state:** `cargo check --all-targets` clean (no warnings);
`cargo test` → **227 passed, 0 failed**; `smoke` and `frontend_smoke` → OK;
release wasm builds (~434 KB) and the manifest lists 43 Konung maps.

## 4. Runtime architecture

The port uses **no `wasm-bindgen`**. `www/app.js` is the shell:

1. fetch `manifest.json`, then fetch each resource;
2. copy bytes into linear memory via `alloc`, hand them over with `load_*`;
3. start either the original frontend (`frontend_boot`) or the game (`boot`);
4. drive the frame loop with `step` and blit the RGBA framebuffer read from
   `frame_ptr`.

`lib.rs` owns a single `Runtime` that holds **both** the original frontend and a
live `PlayableApp` at once, so Escape opens the menu without discarding the game
state and `Resume` reactivates it. `active` selects which side is stepped and
rendered. The framebuffer size is dynamic (`frame_w`/`frame_h`): the frontend
renders 640×480 / 800×600, the game 800×600.

### ABI surface (groups)

- **Memory / resources:** `alloc`, `dealloc`, `load_resource`, `load_profile`, `load_konung`
- **Boot / lifecycle:** `frontend_boot`, `boot`, `frontend_set_resolution`, `should_quit`
- **Loop / frame:** `step`, `frame_ptr`, `frame_w`, `frame_h`, `host_w`, `host_h`
- **Input:** `pointer_down`, `pointer_up`, `pointer_move`, `wheel`, `key_down`, `on_resize`
- **Frontend queries:** `frontend_is_active`, `frontend_state`, `frontend_selected_hero`, `frontend_selected_class`, `frontend_hover`
- **Text overlay:** `text_count`, `text_x`, `text_y`, `text_rgb`, `text_scale`, `text_ptr`, `text_len`
- **Debug (diagnostics):** `debug_*` (player/entity state, decoded MENU/INTR images, HUD preview)

`load_resource(kind, ptr, len)` where `kind` is `KIND_GAME=0`, `KIND_GRAPH=1`,
`KIND_OBJECTS=2`, `KIND_EXE=3`, `KIND_NAMES=4`, `KIND_MENU640/800=5/6`,
`KIND_NEWH640/800=7/8`, `KIND_SAVE640/800=9/10`, `KIND_INTR640/800=11/12`,
`KIND_QUEST=13`, `KIND_SOUNDS=14`.

## 5. Systems overview

- **Coordinates / render pipeline.** Three distinct lattices: base-terrain
  records `116 × 32` with odd-row `+58`; the runtime/Entity grid `58 × 16` with
  odd-column `+29` (`coords::entity_to_local_px`); and the prop grid
  `(grid_y*58 − (grid_x&1)*29, grid_x*16)`. GRAPH placements are identity pixel
  anchors. Collision is the same 320×160 runtime grid as Entity/spawn/transition
  coordinates. All evidence is in the source-of-truth §3–§11.
- **Resources (`res`).** GRAPH/OBJECTS archive decoding, indexed RLE +
  palettes, human/monster animation frames and equipment overlays
  (`compose_human`, `HUMAN_DRAW_ORDER_PEACE`), terrain/prop sprites, `Game.res` variants.
- **Local maps (`loc`).** `Konung.N` layout: dual-layer base terrain,
  320×160 collision, 1000 GRAPH placements, 1000 prop placements (raw `+0x1E/+0x20`).
- **Population (`population`, `actors`).** `Game.N` Group/Entity/Settlement
  parsing; runtime placement from Group rectangle A (≤100 attempts, **no center
  fallback**); A* over the parity-selected direction tables; citizen movement
  with settlement-waypoint ownership.
- **Portals (`edges`, `app::portals`).** `Game.N` transition records; gameplay
  trigger detection uses the trigger rectangle in runtime coordinates. The
  244/245 visual gate props are cosmetic only.
- **Items / HUD (`item`, `hud`, `intr`, `menu_res`).** Item instances decode from
  the loaded `Game.N`; prototype/name tables come from `konung.res`; INTR
  interface archives drive the party panel, inventory strip and AUTO button.
- **Character (`character`, `character_screen`, `hero`).** Hero identity, class
  presets, the stat/skill model and the character-window render.
- **Dialogue/quest (`quest`, `dialog`).** `QUEST_R.RES` (seven fixed blocks)
  decoded by `quest.rs`: `dialogue_id → node`, node/option records, `B6→B3`
  CP866 text, and the B4/B5 condition/action programs. `dialog.rs` drives the
  interface: right-clicking a human NPC with a `dialogue_id` opens the dialogue
  (left-click only moves), hovering/left-clicking options navigates it, rendered
  with `INTR[1]`+`INTR[0]` and the original fonts. Condition/action programs (incl. trade) are no-ops.
  Loaded as `KIND_QUEST=13` (`DIALOG_SYSTEM.md`). The `0x112..0x16E` script
  opcodes are catalogued in `script.rs` (trade/loot `0x113`).
- **Sound (`sound`).** `SOUNDS.RES`/`Sounds_.res` decoded by `sound.rs` (8-byte
  header, `1000×{off,size}` index, data base 8008, raw s16 mono PCM; 391 clips).
  Requested ids are queued and played via WebAudio in the browser shell; the
  original used DirectSound with 22050/19050/25050 Hz buffers
  (`SOUND_SYSTEM.md`). Loaded as `KIND_SOUNDS=14`.
- **Frontend (`frontend/*`).** Original MENU / NEWH / SAVE screens and the
  New-Hero flow.

## 6. Module map

See [`MODULES.md`](MODULES.md) for the exhaustive per-file map.

## 7. Testing & verification

- **Unit tests** (`cargo test`): projection round-trips, collision decode,
  pathfinding/direction tables, item mapping, `Game.N` parsing, INTR structure,
  `QUEST_R.RES` dialogue decode, render-hash oracles. Tests live in
  `<module>/tests.rs` files (one per module).
- **Smoke tests** (Node, no npm deps): `tools/smoke.mjs` (load → boot → frame →
  HUD text → click → graph) and `tools/frontend_smoke.mjs` (full frontend flow).
- **Parity**: `tools/parity.py` (Python reference decoders) is compared against
  `cargo run --release --example dump`. Palette/tile/prop/loc/edges/layout
  hashes match; the Python `human_frame_state` oracle path is the only known
  divergence (oracle-side, not the Rust decoder).
- **Visual tools**: `tools/frame_png.mjs`, `menu_png.mjs`, `intr_png.mjs`,
  `intr_sheet.mjs`, `character_png.mjs`, `host_resize_smoke.mjs`.

## 8. Deployment

`deploy_openkonung.ps1` packages `src` (recursively — every split subtree),
the build scripts, `tools`, `examples`, `www/app.js`, `www/index.html`,
`music/`, **all root `.res` files**, plus the non-`.res` runtime assets
`CLIENT.EXE`, `Game.0/1/2`, and every `Konung.N`. It pushes the tarball and
`deploy_openkonung_remote.sh` to the host; the remote script rebuilds the wasm
inside Docker, stages the assets into `www`, patches the Dockerfile (asset
staging) and `nginx.conf` (uncached js/wasm/json), recreates the container on
`:8000`, and verifies host/container/HTTP hashes and asset sizes.

## 9. Documentation map

| File | Status |
|---|---|
| [`KONUNG_SOURCE_OF_TRUTH.md`](KONUNG_SOURCE_OF_TRUTH.md) | **canonical** R/E contract |
| [`AGENTS.md`](AGENTS.md) | **canonical** agent / build / convention guide |
| [`PROJECT.md`](PROJECT.md) | current architecture & workflow (this file) |
| [`MODULES.md`](MODULES.md) | per-file module map |
| [`DIALOG_SYSTEM.md`](DIALOG_SYSTEM.md) | dialogue/quest (`QUEST_R.RES`) R/E notes |
| [`SOUND_SYSTEM.md`](SOUND_SYSTEM.md) | sound (`SOUNDS.RES`) R/E notes |
| [`QUEST_OPCODES.md`](QUEST_OPCODES.md) | quest/dialogue script opcode reference |
| [`../README.md`](../README.md) | install / build / run / MemPalace quick start |
| [`INSTALL.md`](INSTALL.md) | detailed install and troubleshooting |
| [`MEMPALACE.md`](MEMPALACE.md) | project memory init / mine / MCP |
| `*_FIX*.md`, `PLAYABLE_V*.md`, `*_V*.md` | chronological dev notes; often stale |
| `KONUNG_GAME2_NPC_*` | dated REA evidence bundles |

## 10. Known gaps / open items

Tracked in the source-of-truth §25 (unresolved unknowns) and §28 (binary-only
systems not yet implemented): population growth (`FUN_00420a8c`), collision
occupancy/interaction high bits, the entity-type activity table, the NPC/monster
action machine, settlement→prop linkage, and the WorldObject depth pass. The
remake currently implements idle/walk movement plus the HUD / character /
inventory screens.
