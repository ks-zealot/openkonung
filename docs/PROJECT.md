# OpenKonung — Project Documentation (current state)

> Scope: this document describes **what the project is and how it is built,
> run, tested and deployed today**. Correctness rules and reverse-engineering
> evidence live in [`KONUNG_SOURCE_OF_TRUTH.md`](../KONUNG_SOURCE_OF_TRUTH.md);
> build/agent conventions live in [`AGENTS.md`](../AGENTS.md).
> Per-file responsibilities are in [`MODULES.md`](MODULES.md).

## 1. What this is

A reverse-engineered, browser playable remake of the 1999 game **Konung**. The
remake reads the original shipped data files directly and reimplements the
behavior of the original `konung.res` (PE32/i386) executable.

Two implementations live in the repository:

- `web/src/*.rs` — the canonical **Rust → WebAssembly** browser port.
  **Active development.**
- root `*.py` — legacy Python decoders/viewers kept as **reference oracles**
  (they still decode the original files and back the parity check).

`KONUNG_SOURCE_OF_TRUTH.md` is the binding implementation contract: evidence
levels, the coordinate/render pipeline, exact file layouts, per-subsystem rules
and remaining unknowns. When code and the contract disagree, the contract wins
and the code is re-checked against the original binaries.

## 2. Repository layout

```text
original_install/                      # game install dir (immutable inputs)
  konung.res   CLIENT.EXE              # executables (konung.res is the game)
  Game.0/1/2   Konung.N   LIGHTS.N     # runtime data
  *.RES        music/                  # resources + audio
  KONUNG_SOURCE_OF_TRUTH.md            # canonical R/E contract (Rev 4)
  AGENTS.md                            # agent / build / convention guide
  docs/                                # this documentation
  *.py  *_FIX*.md  PLAYABLE_V*.md      # legacy oracles + historical notes
  deploy_openkonung.ps1                # upload + remote deploy (local side)
  deploy_openkonung_remote.sh          # remote (Docker) build script
  web/                                 # the remake
    src/                               # Rust source (see MODULES.md)
    www/    app.js index.html          # browser shell (hand-written)
            openkonung.wasm            # GENERATED build artifact
            openkonung.pack.zip        # GENERATED packed resources (all .res/exe/music)
            manifest.json              # GENERATED build artifact
    tools/                             # smoke tests, PNG exporters, parity
    examples/dump.rs                   # Rust↔Python parity dump
    examples/pack.rs                   # native resource packer (separate target)
    Cargo.toml  build.ps1  build.sh
    serve.ps1   serve.sh   server.py
```

Treat the original data files and `web/target/` as immutable inputs/artifacts.
`www/openkonung.wasm`, `www/openkonung.pack.zip` and `www/manifest.json` are
generated — never hand-edit.

## 3. Build / run / test (run from `web/`)

| Task | Command |
|---|---|
| Build wasm + regenerate manifest | `.\build.ps1` (Windows) / `bash build.sh` |
| Typecheck all targets | `cargo check --all-targets` |
| Unit tests | `cargo test` (**dev profile**; do **not** use `--release` — `panic = "abort"`) |
| Headless wasm smoke (raw fallback) | `node tools/smoke.mjs` |
| Pack-only acceptance smoke (only `web/www` served) | `node tools/pack_smoke.mjs` |
| Frontend smoke (menu → New Game → New Hero → confirm → game) | `node tools/frontend_smoke.mjs` |
| Settings / KONUNG.CFG smoke | `node tools/settings_smoke.mjs` |
| Lighting / LIGHTS.N smoke | `node tools/lighting_smoke.mjs` |
| Movie / packed-video smoke | `node tools/video_smoke.mjs` |
| Page smoke over HTTP | start `serve.ps1`/`serve.sh`, then `node tools/page_smoke.mjs [url]` |
| Run in a browser | `.\serve.ps1` or `bash serve.sh` → http://localhost:8000/ |
| Rust↔Python parity | `python tools/parity.py [base] [profile] [variant]` vs `cargo run --release --example dump -- <base> [profile] [variant]` |
| Render frames to PNG | `node tools/frame_png.mjs` (also `menu_png.mjs`, `intr_png.mjs`, `intr_sheet.mjs`, `character_png.mjs`) |

`build.ps1`/`build.sh` require the `wasm32-unknown-unknown` target and the
native packer target. The build copies `target/…/openkonung.wasm` into
`web/www/`, runs `cargo run --release --example pack` to write
`web/www/openkonung.pack.zip`, and regenerates `web/www/manifest.json` via
`tools/make_manifest.py`.

**Current verified state:** `cargo check --all-targets` clean; `cargo test` →
**401 passed, 0 failed**; `smoke`, `pack_smoke`, `frontend_smoke`, `combat_smoke`,
`ranged_smoke`, `mob_ai_smoke`, `settings_smoke`, `lighting_smoke` and
`video_smoke` → OK; release wasm builds (~1.01 MB) and the manifest lists 43
Konung maps, 40 LIGHTS.N maps, 19 movies and the pack.

## 4. Runtime architecture

The port uses **no `wasm-bindgen`**. `web/www/app.js` is the shell:

1. fetch `manifest.json`; if it advertises `openkonung.pack.zip`, fetch that and
   call `load_pack` (ZIP → `bincode` → one `Loader`); otherwise fetch each
   original resource and hand it over with `load_*` (fallback, `?nopack=1`);
2. copy bytes into linear memory via `alloc` (raw path) or let `load_pack`
   consume them;
3. start either the original frontend (`frontend_boot`) or the game (`boot`);
4. drive the frame loop with `step` and blit the RGBA framebuffer read from
   `frame_ptr`. Packed music is served from `pack_music_*` as `Blob` URLs.

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
- **Config:** `config_load`, `config_write`, `config_revision`
- **Movies:** `start_intro_video`, `video_pending`, `video_name_ptr/len`, `video_finish`
- **Debug (diagnostics):** `debug_*` (player/entity state, decoded MENU/INTR images, HUD preview)

`load_resource(kind, ptr, len)` where `kind` is `KIND_GAME=0`, `KIND_GRAPH=1`,
`KIND_OBJECTS=2`, `KIND_EXE=3`, `KIND_NAMES=4`, `KIND_MENU640/800=5/6`,
`KIND_NEWH640/800=7/8`, `KIND_SAVE640/800=9/10`, `KIND_INTR640/800=11/12`,
`KIND_QUEST=13`, `KIND_SOUNDS=14`, `KIND_FLAMES=15`, `KIND_LIGHTS=16`.
Per-map `LIGHTS.N` payloads use `load_light_map(id, ptr, len)`. Pack format 3
also carries 19 H.264 movies through `pack_video_*`.

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
  with settlement-waypoint ownership. **Mob AI (`app/combat/ai.rs`):**
  `FUN_00412C30` group relations every 16 ticks, exact `FUN_004245B4` wake,
  autonomous target acquisition (shared/unengaged/nearest/8-neighbour),
  citizen combat without wander frame reset, and the M/O `Entity+0x12` walk
  delay.
- **Portals (`edges`, `app::portals`).** `Game.N` transition records; gameplay
  trigger detection uses the trigger rectangle in runtime coordinates. The
  244/245 visual gate props are cosmetic only.
- **Items / HUD (`item`, `hud`, `intr`, `menu_res`).** Item instances decode from
  the loaded `Game.N`; prototype/name tables come from `konung.res`; INTR
  interface archives drive the party panel, inventory strip and AUTO button.
- **Character (`character`, `character_screen`, `hero`).** Hero identity, class
  presets, the stat/skill model and the character-window render.
- **Frontend (`frontend/*`).** Original MENU / NEWH / SAVE screens and the
  New-Hero flow.

## 6. Module map

See [`MODULES.md`](MODULES.md) for the exhaustive per-file map.

## 7. Testing & verification

- **Unit tests** (`cargo test`): projection round-trips, collision decode,
  pathfinding/direction tables, item mapping, `Game.N` parsing, INTR structure,
  render-hash oracles. Tests live in `<module>/tests.rs` files (one per module).
- **Smoke tests** (Node, no npm deps): `tools/smoke.mjs` (load → boot → frame →
  HUD text → click → graph) and `tools/frontend_smoke.mjs` (full frontend flow).
- **Parity**: `tools/parity.py` (Python reference decoders) is compared against
  `cargo run --release --example dump`. Palette/tile/prop/loc/edges/layout
  hashes match; the Python `human_frame_state` oracle path is the only known
  divergence (oracle-side, not the Rust decoder).
- **Visual tools**: `tools/frame_png.mjs`, `menu_png.mjs`, `intr_png.mjs`,
  `intr_sheet.mjs`, `character_png.mjs`, `host_resize_smoke.mjs`.

## 8. Deployment

`deploy_openkonung.ps1` packages `web/src` (recursively — every split subtree),
the build scripts, `web/tools`, `web/examples`, `www/app.js`, `www/index.html`,
`music/`, **all root `.res` files**, plus the non-`.res` runtime assets
`CLIENT.EXE`, `Game.0/1/2`, and every `Konung.N`. It pushes the tarball and
`deploy_openkonung_remote.sh` to the host; the remote script rebuilds the wasm
inside Docker, stages the assets into `web/www`, patches the Dockerfile (asset
staging) and `nginx.conf` (uncached js/wasm/json), recreates the container on
`:8000`, and verifies host/container/HTTP hashes and asset sizes.

## 9. Documentation map

| File | Status |
|---|---|
| [`KONUNG_SOURCE_OF_TRUTH.md`](../KONUNG_SOURCE_OF_TRUTH.md) | **canonical** R/E contract |
| [`AGENTS.md`](../AGENTS.md) | **canonical** agent / build / convention guide |
| [`docs/PROJECT.md`](PROJECT.md) | current architecture & workflow (this file) |
| [`docs/MODULES.md`](MODULES.md) | per-file module map |
| `README.md` | stale historical viewer notes |
| `*_FIX*.md`, `PLAYABLE_V*.md`, `*_V*.md` | chronological dev notes; often stale |
| `KONUNG_GAME2_NPC_*` | dated REA evidence bundles |

## 10. Known gaps / open items

Tracked in the source-of-truth §25 (unresolved unknowns) and §28 (binary-only
systems not yet implemented): population growth (`FUN_00420a8c`), collision
occupancy/interaction high bits, the entity-type activity table, the NPC/monster
action machine, settlement→prop linkage, and the WorldObject depth pass. The
remake currently implements idle/walk movement plus the HUD / character /
inventory screens.
