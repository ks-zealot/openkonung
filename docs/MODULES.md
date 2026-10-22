# OpenKonung — Module Map

Per-file responsibilities of `web/src`. Each `*.rs` file at the top level is a
module root; a matching sibling directory holds its subsystems and its
`tests.rs`. Systems were split out of the former monoliths (`app.rs`, `res.rs`,
`actors.rs`, `lib.rs`, …); tests live in `<module>/tests.rs`.

## Crate root

| File | Responsibility |
|---|---|
| `lib.rs` | Crate root: `pub mod` list; the wasm ABI entry surface, delegated to `abi/*`. |
| `app.rs` | `PlayableApp` state machine: struct + fields, `new()`/`step()` orchestration, shared helpers. |
| `res.rs` | Resource container (`Res`) and decoding entry points; re-exports `res/*`. |
| `actors.rs` | Runtime population API + frame/tick constants; re-exports `actors/*`. |
| `population.rs` | `Game.N` Group/Entity/Settlement tables; re-exports `population/*`. |
| `resource_pack.rs` | Packed archive model (`ResourcePack` + `bincode`), validation, `Loader` application, minimal ZIP reader/native writer; re-exports `resource_pack/*`. |
| `character.rs` | Hero/class/build/player model; re-exports `character/*`. |
| `loc.rs` | `Konung.N` map decode: base terrain, 320×160 collision, GRAPH/prop placements, gates. |
| `edges.rs` | `Game.N` transition graph (`+0x29600`), trigger rectangles, location graph. |
| `coords.rs` | Canonical coordinate transforms (terrain/entity/prop) + movement direction tables. |
| `render.rs` | World surface renderer and sprite layer/culling (`World`, `WorldSprite`, `LayerCounts`). |
| `anim.rs` | Stance/action mapping and battle-flag helpers. |
| `hero.rs` | Hero identity templates (Dragomir/Mikhail/Wolf) and identity field decoding. |
| `character_screen.rs` | Character window (`INTR[2]`) render: equipment/stat/name layout tables. |
| `hud.rs` | Gameplay HUD composition; re-exports `hud/*`. |
| `intr.rs` | INTR interface-image archive decoder; re-exports `intr/*`. |
| `item.rs` | Item instance decode + prototype/name/rating resolution from `konung.res`. |
| `menu_res.rs` | Native MENU/NEWH/SAVE frontend-image RES decoder. |
| `font.rs` | Original proportional bitmap fonts embedded in `konung.res`. |
| `names.rs` | CP866 name decoding and name/class table lookup. |

## `app/` — live game subsystems

| File | Responsibility |
|---|---|
| `camera.rs` | Camera follow/zoom/pan, window↔virtual mapping, resize handling. |
| `portals.rs` | Cosmetic transition↔visual-gate binding and matching (map ring/label; `T` overlay). |
| `population.rs` | `Game.N` population integration (placement, actor sprite collection, entity debug). |
| `player.rs` | Player targeting/pathing, per-cell stepping, protagonist animation frame. |
| `text.rs` | Text overlay helpers (label, wrap, clamp). |
| `world_draw.rs` | `draw_detail`: terrain + props + actors + world overlays compositing. |
| `debug_overlay.rs` | Debug overlays: portals (`T`), render layers (`L`), population (`P`). |
| `graph.rs` | World-graph view (`H`): spring layout, fit, draw, labels. |
| `input.rs` | Pointer/key/wheel handling, hover, party-cell hit-testing, character-screen toggles. |
| `tests.rs` | App-level tests (character screen, menu/click-through, portal matching, text wrap). |

## `res/` — resource decoding

| File | Responsibility |
|---|---|
| `graph.rs` | GRAPH terrain tiles and GRAPH layout parsing. |
| `objects.rs` | OBJECTS props/monsters: object index, prop sprites, monster animation frames. |
| `human.rs` | Humanoid frames, peace/battle equipment overlays, `compose_human`, `paper_doll_frame`. |
| `raster.rs` | `Image`, indexed RLE, palette conversion, `blit`/`fill_*`/`line`, byte readers. |
| `exe.rs` | EXE profile mapping (virtual address ↔ file offset) for embedded tables. |
| `tests.rs` | Human/object frame and overlay tests. |

## `actors/` — runtime population

| File | Responsibility |
|---|---|
| `placement.rs` | Deterministic `(0,0)` spawn placement from Group rectangle A; RNG; `PopulationBuild`. |
| `pathfind.rs` | Passability, wander target selection, A* `find_path`. |
| `motion.rs` | `RuntimeActor`/`ActorMotion`, per-tick `update_population`, depth sort keys. |
| `tests.rs` | Pathfinding, blocked-goal, straight-walk, placement tests. |

## `population/` — `Game.N` tables

| File | Responsibility |
|---|---|
| `types.rs` | `Group`/`Entity`/`EntityStats`/`GameTables`/`Settlement`/`Waypoint` + accessors. |
| `parse.rs` | Parsers for the Group/Entity/Settlement tables plus diagnostics. |
| `tests.rs` | Parsing and real-`Game.1` regression tests. |

## `character/` — hero / class model

| File | Responsibility |
|---|---|
| `class.rs` | `CharacterClass`, class presets, `PrimaryStat`, specializations, `stat_cost`. |
| `build.rs` | `CharacterBuild`, skills, `INITIAL_SKILLS`, `max_load`/`max_party`. |
| `player.rs` | `PlayerCharacter` (runtime hero), starting-item attach, portrait/status. |
| `tests.rs` | Preset, stat-cost, specialization and loadout tests. |

## `frontend/` — startup UI

| File | Responsibility |
|---|---|
| `mod.rs` | `Frontend` state, input translation, screen dispatch, text overlay. |
| `screens.rs` | Per-screen draw routines (Settings/Save-Load/MainMenu/NewHero/region debug). |
| `layout.rs` | Original frontend layout tables (menu/newh regions, resolutions, hit rects). |
| `main_menu.rs` | Main-menu visibility and hit-test. |
| `new_hero.rs` | New-Hero state (hero/class selection, confirm, image mapping). |
| `tests.rs`, `layout/tests.rs` | Frontend and layout tests. |

## `hud/` — gameplay interface

| File | Responsibility |
|---|---|
| `layout.rs` | HUD sprite layout, inventory-strip slot math, party grid. |
| `compose.rs` | `draw_game_hud` composition (party panel, inventory strip, AUTO, hover). |
| `tests.rs` | HUD layout tests. |

## `intr/` — interface archives

| File | Responsibility |
|---|---|
| `roles.rs` | `IntrRole` catalogue and `role_of` for every occupied INTR index. |
| `tests.rs` | INTR decoding tests. |

## `constants/` — centralised magic values

| File | Responsibility |
|---|---|
| `mod.rs` | Re-exports the constant submodules. |
| `coords.rs` | Grid/lattice steps and tile dimensions (`58/16/29`, `116/32/58`, tile `114×64`). |
| `konung_layout.rs` | `Konung.N` block offsets/sizes. |
| `game_layout.rs` | `Game.N` table offsets/strides/counts, `Game.res` variant offsets, transition layout. |
| `exe_tables.rs` | `konung.res` VA table bases (item icon/name/armor, font offsets). |
| `timing.rs` | Frame budget (`60 ms`), `LOGIC_TICK_HZ`, path/wander limits, flags. |
| `keys.rs` | Key codes. |
| `render.rs` | Virtual framebuffer dimensions. |

Original modules re-export the moved names (`pub use crate::constants::…`) so
existing paths such as `crate::res::GRID_X_STEP` and `crate::loc::KONUNG_SIZE`
keep working.

## `abi/` — wasm ABI (`#[no_mangle] extern "C"`)

| File | Responsibility |
|---|---|
| `memory.rs` | `alloc`, `dealloc`, byte-copy helper. |
| `resource.rs` | `Loader`, `load_resource`/`load_profile`/`load_konung`, kind constants, frontend asset build. |
| `pack.rs` | `load_pack`, pack build-id and `pack_music_*` accessors for the browser shell. |
| `lifecycle.rs` | `Runtime`, `ActiveMode`, boot / start-game / menu switching, `apply_host`. |
| `frame.rs` | `step`, `frame_ptr`/`frame_w`/`frame_h`, `should_quit`, `on_resize`, host size. |
| `input.rs` | Pointer/wheel/key forwarding into the active side. |
| `frontend_api.rs` | Frontend state / hero / class / hover queries. |
| `debug.rs` | Debug state images, decoded MENU/INTR previews, HUD preview, text-overlay exports. |

## Non-source entry points

| Path | Responsibility |
|---|---|
| `web/www/app.js` | Hand-written browser shell (manifest fetch, memory loading, loop, input, blit). |
| `web/www/index.html` | Canvas host page. |
| `web/examples/dump.rs` | Rust↔Python parity dump (`tools/parity.py` mirrors it). |
| `web/examples/pack.rs` | Native resource packer (writes `openkonung.pack.zip`; never linked into wasm). |
| `web/tools/smoke.mjs` | Headless wasm smoke test (raw per-file fallback). |
| `web/tools/pack_smoke.mjs` | Pack-only acceptance smoke (only `web/www` served). |
| `web/tools/frontend_smoke.mjs` | Full frontend-flow smoke test. |
| `web/tools/page_smoke.mjs` | Served-bundle HTTP smoke test. |
| `web/tools/frame_png.mjs`, `menu_png.mjs`, `intr_png.mjs`, `intr_sheet.mjs`, `character_png.mjs` | Diagnostic PNG exporters. |
| `web/tools/parity.py` | Python reference decoders for parity. |
| `web/tools/make_manifest.py` | Manifest generator (run by `build.ps1`/`build.sh`). |
