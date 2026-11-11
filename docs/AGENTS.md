# AGENTS.md

## What this repo is
A reverse-engineered remake of the 1999 game **Konung**, driven directly by the
original data files. Put them in `assets/` (recommended), the repository root,
or the directory above the repository (`Game.res`, `GRAPH.RES`, `Objects.res`,
`Game.0/1/2`, `Konung.N`, `CLIENT.EXE`). The build writes one
`www/konung.zip.pack`; later builds reuse it without touching any `.res`/`.exe`.

- `src/*.rs` — the canonical Rust/wasm browser port. **Active development.**
- The legacy Python decoders/viewers in the original install directory are kept
  as reference + parity oracles.

`KONUNG_SOURCE_OF_TRUTH.md` is the binding implementation contract. When code and
the document disagree, re-check the original binaries, then fix the code; do not
invent coordinate scaling, clamping, fallbacks, or reinterpretations. Follow its
evidence hierarchy and answer its §14 "CHECK" list in your work log.

The `docs/*_FIX*.md` / `docs/PLAYABLE_V*.md` notes are chronological and often
stale; trust `KONUNG_SOURCE_OF_TRUTH.md` and the code over them.

## Commands (run from the repository root)
- From-scratch bootstrap (toolchain check + build): `.\setup.ps1` (Windows) or
  `bash setup.sh`.
- Build wasm + resource pack + manifest: `.\build.ps1` (Windows) or `bash build.sh`.
  Requires the `wasm32-unknown-unknown` target. Outputs `www/openkonung.wasm`,
  `www/konung.zip.pack` and `www/manifest.json`. The pack is written by the separate native target
  `cargo run --release --example pack -- --base <install-dir>
  --manifest www/manifest.raw.json --out www/konung.zip.pack` (see
  “Packed resources” below). The first build reads the original files from
  `assets/`, the repository root or the parent directory; later builds reuse
  the pack and never read `.res`/`.exe` (pass `-Repack`/`--repack` to rebuild).
- Typecheck all targets incl. the `dump` and `pack` examples:
  `cargo check --all-targets`
- Unit tests: `cargo test` (dev profile).
  **Do not use `--release`**: `[profile.release]` sets `panic = "abort"`, which
  breaks the test harness.
- Headless wasm smoke test (requires a prior build; Node, no npm deps):
  `node tools/smoke.mjs`
- Headless save/load smoke (snapshot round-trip and fresh-runtime load):
  `node tools/save_smoke.mjs`
- Headless legacy interop smoke (exact `0xB07A4` `KONUNG.SAn` export/import +
  `KONUNG.SAV` CP866 names): `node tools/legacy_smoke.mjs`
- Player-feedback visual/regression smoke (hotkeys, item tooltip, attack
  animation, merchant stock, cursor, sound queue, engage/retaliation, monster
  aggro; writes `tools/feedback/*.png`): `node tools/feedback_smoke.mjs`
- Layout-independent keymap smoke (Cyrillic `event.key` -> physical VK):
  `node tools/keymap_smoke.mjs`
- Headless melee combat smoke (attack order -> animation -> death -> corpse
  loot -> pickup): `node tools/combat_smoke.mjs`
- Headless autonomous mob AI smoke (group activity -> acquisition -> row-5
  attack -> leader damage): `node tools/mob_ai_smoke.mjs`
- Headless shadow smoke (ground-shadow span pass: `shadows` config gate, span
  count, darken-only frame diff): `node tools/shadow_smoke.mjs`
- Headless settings/config smoke (28-byte KONUNG.CFG, settings hit rects,
  toggles, Ok persist, Cancel snapshot restore, frontend video mode):
  `node tools/settings_smoke.mjs`
- Headless lighting smoke (packed LIGHTS.RES + 40 LIGHTS.N maps, day/night
  ambient darkening): `node tools/lighting_smoke.mjs`
- Headless movie smoke (19 transcoded/packed MP4s, intro + trigger handshake):
  `node tools/video_smoke.mjs`
- Headless ranged smoke (arrow flight -> ammo/bow wear -> hit/dodge -> kill
  rewards -> arrow-stack loot -> prop settlement branch):
  `node tools/ranged_smoke.mjs`
- Headless trade smoke (role-2 restock -> buy -> sell/destroy -> cancel):
  `node tools/trade_smoke.mjs`
- Frontend smoke test (menu -> New Game -> New Hero -> confirm -> game):
  `node tools/frontend_smoke.mjs`
- Pack-only acceptance smoke (serves only `www`, loads
  `konung.zip.pack` via `load_pack`, proves no `.res`/`.exe`/music download):
  `node tools/pack_smoke.mjs`
- Page-level smoke over HTTP (served bundle + cache-busting; prints the content
  build id and post-boot `frame_w x frame_h`): start `serve.ps1`/`serve.sh`, then
  `node tools/page_smoke.mjs [http://localhost:8000/]`. It drives the raw
  per-file path; the browser shell prefers the pack automatically and
  `?nopack=1` forces the fallback in the browser.
- Render frames to PNG for visual checks: `node tools/frame_png.mjs`
- Decode original menu RES images / frontend frames to PNG (diagnostic only;
  uses the runtime decoder via the wasm ABI): `node tools/menu_png.mjs`
  (writes `tools/menu_dump/`).
- Run in browser: `.\serve.ps1` or `bash serve.sh` -> http://localhost:8000/
  (`server.py` serves `www` at `/` and auto-detects the original install dir for
  the `?nopack=1` fallback; the normal page loads only `konung.zip.pack`.)
- Rust↔Python parity: `python tools/parity.py [base] [profile] [variant]`
  vs `cargo run --release --example dump -- <base> [profile] [variant]`.
  Both print FNV hashes and must match byte-for-byte.

## Wiring
- `src/lib.rs` is a raw `#[no_mangle]` wasm ABI (no wasm-bindgen): JS copies
  resource bytes into linear memory via `alloc`/`load_*`, drives `step`, reads the
  800x600 RGBA framebuffer from `frame_ptr`. `www/app.js` is the browser shell.
  The runtime is either the original frontend (`frontend_boot`) or the game
  (`boot`); `frame_w`/`frame_h` are dynamic (frontend 640/800, game 800).
- **Packed resources (implemented).** `www/konung.zip.pack` is a standard ZIP
  produced by the native-only `examples/pack.rs` target; `bundle.bin` is a
  `bincode`-serialized `resource_pack::ResourcePack` (raw original bytes tagged
  with the existing `KIND_*` selectors + Game.N profiles + 43 Konung.N maps +
  the 10 Ogg tracks), `pack.json` is a human-readable index, and
  `manifest.raw.json` is the embedded per-file manifest used by
  `make_manifest.py --from-pack` (so later builds never rescan `.res`/`.exe`).
  The wasm ABI
  `load_pack(ptr,len)` (`abi/pack.rs`) finds/inflates `bundle.bin` with a minimal
  ZIP reader (`flate2`, CRC-verified, no ZIP64), deserializes it, validates the
  required resources, and atomically fills the same `Loader` used by
  `load_resource`/`load_profile`/`load_konung`. Decoders and rendering are
  unchanged. `manifest.pack` advertises the archive; `www/app.js` loads only the pack and
  refuses to start when it is absent or rejected. `?nopack=1` is the explicit
  development escape hatch that loads the per-file downloads instead. **Acceptance:** when the pack loads, the
  page requests only `manifest.json`, `openkonung.wasm`, `app.js`/`index.html`
  and `konung.zip.pack` — no `.res`, `.exe`, `Game.N`, `Konung.N` or
  `/music/` download. Music bytes are exposed by `pack_music_*` and played from
  `Blob` URLs. Build id = hash(shell files + pack); pack build id =
  FNV(resource names + bytes).
- Logic: `app.rs` (state/input/camera/portals), `render.rs`+`res.rs` (decoding,
  surfaces), `loc.rs` (Konung.N), `edges.rs` (Game.N transition graph),
  `coords.rs` (isolated unresolved `0x438A78` transform — not the render
  projection), `names.rs`.
- Interface assets: `intr.rs` decodes the original `INTR640R.RES`/`INTR800R.RES`
  archives (8-byte header + 512-entry `{data_offset,palette_offset}` table, 275
  occupied indices 0..=274; per-entry GRAPH palette, reusing `res.rs`'s indexed
  RLE). `Res::load_intr`/`Res::intr_sprite` attach it at game start (ABI kinds 11/12)
  with load-time structural validation and lazy per-index decode
  (`tools/intr_png.mjs` is the diagnostic exporter).
- HUD: `hud.rs` composes the original gameplay interface over the world: the
  whole-screen `INTR[3]` overlay at (0,0), the `INTR[17]` inventory strip
  (executable-derived: `step = INTR[17].w − 1 = 69`; the side margin is
  `INTR[18].w = 20`, so visible slots = `(span+2−2·20)/69`: **800 -> 9 slots
  origin (159,486); 640 -> 7 slots origin (134,375)**, flanked by the original
  `INTR[18]/[20]` scroll-arrow caps) with the protagonist's item icons, and a
  2×5 = 10-cell party panel, and the AUTO experience-spend button
  (`INTR[169]/[170]`, selected by `Entity+0x03 & 0x02`, traced position
  (644,379)/(515,303); `FUN_0041079C` auto-raises level/stats while the bit is
  set - it is **not** auto-combat).
  `hud::portrait_id` ports the selector at VA `0x42C730`. Called from
  `app.rs::draw_detail` after world overlays. **Lower-panel hover text is
  implemented** (`app/hover.rs` + `hud::draw_hover_text`): the original hover
  branch (`FUN_0042260C` `0x423CD2`, world mode only) selects a ground-marker
  message (`0x45C31C`), the actor panel `FUN_00430958`, a fixed HUD-button
  description (`FUN_0042E9CC`), or the item detail `FUN_0042F5F8`, and draws it
  with `FUN_0042BD8C` (small font `0x58BC34`) **left aligned at
  `viewport.left+10`, `viewport.bottom+5`**, wrapped to `viewport width-16`,
  15 px line advance — 800×600 → (150, 560), 644 wide; 640×480 → (122, 449),
  512 wide. The actor panel is `Name [Nickname], Class, Уров:N, Здор:M` for
  humans (party adds `, воин дружины` + poison + `Хар/Сил/Лов/Вын`; an active
  hostility group adds `, враг`; otherwise the `FUN_0041182C` profession or the
  healing/smithing/trade/building/identification skill labels), and
  `Name, Уров:N, Здор:M` for monsters. The item detail includes
  damage/armor, wear, weight, poison, weapon modifier, quality-condition
  bonuses/requirements and the prototype requirement. The character screen and
  trade window still use `app/tooltip.rs`, which keeps the recovered name and
  adds a data-derived property block (weight/price/durability/combat value/
  range/requirements). The shipped data has no flavour-text table.
  `intr::role_of`/`IntrRole` catalogue every occupied INTR index (HUD, character
  window, slot, arrows, AUTO, item icons 22..=168, portraits 200..=274, window
  frames/panels/controls); non-HUD sprites are catalogued + accessible only (not
  drawn until their screens exist). `tools/frame_png.mjs` writes the HUD
  screenshots, `debug_render_intr_hud` gives an isolated 640×480 preview, and
  `tools/intr_sheet.mjs` dumps all 275 sprites.
- HUD button strip (**implemented**): the empty lower-left panel is the original
  seven-icon array `0x81CF68[0..7]`, drawn by `FUN_0042CA44` from position
  table `0x45AEA0` (640/800 variants). Slot 0 is the active weapon plaque
  (`INTR[16]` sword / `[8]` axe / `[9]` bow by `Entity+0x12`); the six buttons
  are stance `10` battle / `15` peace (`Entity+0x03 & 4`), map `14`, halt `11`,
  engage `7`, quest log `12`, character screen `13`. Clicks use the exact
  `FUN_00438900` row/run tables `0x456FC0`/`0x4523E4` (640) and
  `0x457740`/`0x45437C` (800), generated into `hud/buttons_data.rs` by
  `tools/gen_ui_tables.py`. Codes 1..7 map to the original handlers: weapon-slot
  cycle (`FUN_004216DC(-1)`), stance (`FUN_00421554`), map (`FUN_0042CBD0(4)`),
  halt (`FUN_0042246C`), engage (`FUN_00421458`), quest log (`FUN_004242BC`),
  character (`FUN_00424330`). M/Q bind the map/quest toggles; Escape closes.
- World map (**implemented**, mode 5): `world_map.rs` tiles `INTR[5]` stone over
  the viewport, draws `INTR[4]` parchment (651x478 @800 / 528x388 @640) and
  masks `INTR[179]` (502x361) through the original run table `0x450EBC`
  (row offsets `0x456A1C`, 361 rows), generated into `world_map/data.rs`.
  Map origin = viewport + (80,60) @800 / (13,13) @640; the left panel and bottom
  bar stay visible (`FUN_00428390` covers the viewport only). Discovered mask is
  the 56-byte `0x81CF80`; a class-11/subtype-0 map item reveals one random
  undiscovered, non-reserved location (`FUN_00414DE4` case 0, reserved table
  `0x45AE30`) and is consumed. New games do **not** auto-discover; loading
  restores the mask. Map clicks use `FUN_004381C8` mode 5 and return
  `index<<8|0x103`; travel finds the transition record whose destination
  location (`+0x04`) equals the run value and loads it at arrival `+05/+07`
  with facing `+02`. Debug ABI: `debug_reveal_all_locations`,
  `debug_set_map_open`, `debug_current_map`, `debug_map_location_point`;
  headless check `tools/map_smoke.mjs`.
- Quest log (**implemented**, mode 2): `app/quest_log.rs` draws the original
  stone fill + `INTR[1]` border and lists every active runtime `B7` slot with
  its `+0x02` journal line (`QuestData::journal_line`/`line_text`) using the
  small font at the recovered prompt column, 15 px line advance. Book icon or
  `Q` toggles it.
- Character screen: `character_screen.rs` renders `INTR[2]` at (140,0)/(112,0)
  clipped to the gameplay region (party panel + inventory strip stay visible),
  draws equipped icons at the **absolute** original per-resolution slot tables
  (main storage→visual `[3,4,3,0,1,2]` + slot 0/2 overlap branch; additional;
  special), the name/class (CP866 tables `0x45E71C`/`0x45ADC0`) with the large
  font, and every stat value with the small font (`0x58BC34`) at the **absolute**
  per-resolution anchors recovered from `FUN_0042cbd0` case 3 (value table
  `0x465C54`, records `{x640,y640,x800,y800}`; secondary table `0x465B34`). The portrait in party cell 0 uses
  `hud::portrait_id` from the actual `+0xD6`/`+0x04`/`+0x5C` fields.
  **Navigation (verified from `konung.res`):** the original opens the screen via
  interface **mode 7** (`FUN_00430d6c`, reached from a UI command/action table —
  not a mouse portrait click; a mode-0 portrait click only *selects* the member
  and is consumed). The remake opens it with **`C`** and closes with **Escape**
  (`app.rs`); camera-follow moved to **`V`**. While open all clicks are consumed
  (no world click-through). New-game loadout is resolved: the final equipment/inventory
  come from the loaded `Game.N` `Entity[0]` (the `0x437E21` restore copies
  identity/stats only), not the creation-screen arrays; per-hero identity is
  `D4/D6/D8`. `tools/character_png.mjs` captures all three heroes. Rendered
  values: Опыт/Уровень, base+current Харизма/Сила/Ловкость/Выносливость,
  Меткость/Фехтование, the five skills, Золото, Свободный опыт, plus health
  `(hp_internal+15)/16`, poisoning `periodic_damage (+0x50)`, armor
  `base_armor (+0x4E)` + item rating over slots 3..=5, attack (`+0x12` weapon
  slot rating; x87 rounding term omitted), and current weight (`FUN_0041b7e4`
  item-weight sum `/1000`). The «поднять» column uses `FUN_00410b08` = the tier
  cost (`character::stat_cost`, identical to its `0x45AFB0` table) shown only
  when `available_xp` covers it (blank for a fresh character); its per-class
  requirement tables (`0x45B0AB`) and the icon-only «Модиф. уровень» slot are
  not rendered. The screen is now editable: `inventory.rs` owns the cursor /
  held-item model and `app/inventory.rs` routes clicks/drags; clicking a party
  portrait while the screen is open switches the edited member (`_81D560`).
- Inventory/crafting: `inventory.rs` holds `InventoryOwner` (protagonist or
  companion), `ItemLocation`, weight (`FUN_0041B7E4`, class-12 stack × quantity),
  capacity `(endurance_base*1000)/3+10000`, front-insert `FUN_0042462C`, class
  gates (`FUN_0041E450`/`E6BC`), broken sweep (`class 0xFF` or durable `<=0`),
  and player alchemy. Craft = held class-9 item + target class-9 item through the
  33-row table at `0x450DF4` (`FUN_0041BB70`): held item becomes the product,
  target becomes `out_b` (`3` empty jar / `0xFF` consumed), quality uses the
  actor's healing skill (`+0x48`) and gains the row's skill delta (cap 500).
  `FUN_0041DAB0` potion effects (subtypes 4..13) are implemented in
  `inventory::use_item` with the `+0xBC..+0xC2` saved stats and `+0xCA/+0xCE`
  temporary-effect fields. Cursor operations: click to lift, click to place,
  left-click world to drop (`FUN_004138C8`), right-click strip to use, drag with
  the left button for equipment/member/ground drops. Automatic stacking is
  class-12 only (faithful). The full `FUN_0041D048` settlement block is
  implemented (`app/settlement_pass.rs`, once per 16 original frames,
  `(frame + 7) & 0xF == 0`, for **all 20 settlements**): the worker count
  (`free = visual<3 visible`, minus role apprentices and waypoint owners; 0 when
  `Group+0x1D == 1`), the no-worker clear, the `FUN_00416670` waypoint claim
  (armed foreign party inside rectangle A sets `Group+0x1A = 0`/`+0x1D = 1`),
  the `FUN_0041B658` power-ratio defeat (`>= 2.0`, `DEFEAT<loc>.FLM`), the
  `+0x14` money-pool event (`0x2760` passes, `sqrt(leader.trade)`), the target
  timer/state pass (`app/settlement_targets.rs`, live `FUN_004172AC` prop
  update, `local_30`), role-2/role-4 restock (`app/settlement_trade.rs`), and
  the `FUN_00417F84` voivode/barracks training (`app/settlement_training.rs`,
  skipped when `local_30` is set). Producer slots are the settlement waypoint
  owners (`+0x3DA/+0x3E2/+0x3EA/+0x3F2` = `waypoint[1..4].owner`), exposed as
  `Settlement::producer(slot)`/`voivode()`; `SettlementTrade::prod_role*` only
  mirrors them for save/legacy export. Waypoint `+1` is the **authored role
  dialogue id**: `0x14C`/`0x159`–`0x15C` copy it onto the settled specialist
  (`Entity+0xD0`) and clear the apprentice role bits, so the new healer/merchant/
  smith/voivode/elder is talkable. Recruited companions remain a `party` overlay
  over their source `tables.entities` row, so settlement scans subtract
  `party_ids` from worker/defender/building/apprentice/restock/tribute counts.
  `Settlement+0x14` is `Settlement::progress_14` (u16 is enough: it resets at
  `0x2760`). Combat death does **not** clear a role waypoint; only `0x16B`,
  `0x123` and waypoint assignment do. See `SETTLEMENT_SYSTEM.md` and
  `SETTLEMENT_TRAINING.md`.
- Animation/actions: `anim.rs` holds the original stance/action mapping.
  `Entity+0x03` bit `0x04` = battle stance; the mode-paired actions are idle
  `0/6`, walk `1/7`, attack `2/8`, death `3/9` (`4`/`5` are unpaired specials).
  Verified from `konung.res`: `FUN_00414A34` idle, `FUN_004142D4` walk,
  `FUN_004123BC`/`FUN_00415FC8` attack, `FUN_0041D048` death. The human GRAPH
  animation table is `block + action*0xD0 + direction*0x1A` (count `u16` at
  `+8`, frame ids at `+0x0A`; **10 action rows**, 8 directions); the
  object/monster table in `Objects.res` is `block + row*0x130 + direction*0x26`
  with **6 rows** (actions `6..=11` alias `0..=5`); the per-frame screen anchor
  is a **separate** table, `block + 0x1728 + row*0x480 + direction*0x90 + fi*8`
  (gx at `+4`, gy at `+6`; 18 slots per direction — `FUN_00414078` object branch
  `0x41410a`-`0x414143`), **not** indexed by the animation frame id. `res.rs`
  `actor_frame_state`/`action_cycle_frames` resolve both; `actors.rs`
  `ActorMotion.action`/`frame` advance the **walk** animation per tick and
  **hold** the idle frame (the original `FUN_004123BC` reaches its idle
  cases `0`/`6` only for `flags_a & 0x40 == 0` entities, or via `FUN_00412E00`
  reporting an active move; every actor in the shipped active population is
  `flags_a = 0x40`, so idle mobs stay still). Authored idle action
  is `Entity+0x00` (peaceful NPCs are `6`, battle `0`); `3`/`9` are the death
  rows, and `Entity+0x10`/`Entity+0xD7` are the current frame / action frame
  count (`FUN_0041435c`, wrapped by `FUN_00413de4`). The remake keeps the
  authored row in `Entity.kind` and the live row in `ActorMotion.action`; use
  the latter (via `runtime_action`) for runtime death checks. `SPACE` runs the
  original stance command (`FUN_00421554`) over the current party selection:
  every selected, living unit enters/leaves battle according to the first
  selected unit's stance; unselected members are untouched (`TAB` remains a
  remake alias). `A` runs `FUN_00421458` party engage: all party units enter
  battle stance and the **persistent engage order** (`Entity+0x02 & 0x01`, with
  `Entity+0x06` cleared) is written on every living member; the original
  assigns no concrete target. Each ordered unit then re-acquires through
  `FUN_0041E2D4` on its next tick (see below). Movement is preserved (the
  original does not halt). `F` runs
  `FUN_0042246C` party halt; camera fit moved to `Home`. Keys `1/2/3` select
  weapon slots `2/0/1` (`FUN_004216DC`), `I`/`C` toggle the character screen,
  `M`/`Q` the map/quest log, `V` camera follow. The browser maps physical
  `event.code` values (`www/keymap.js`), so Russian/Cyrillic layouts still
  dispatch the original VK codes; `F1/F2` are the resolution diagnostics and
  `Ctrl+I`/`Ctrl+E` the legacy snapshot import/export. `FUN_0041EB48` adjacent
  retaliation is implemented for the leader and companions: an idle unit
  acquires an adjacent object-family actor already targeting it and enters
  battle stance, so a party walking through an aggroed group fights back.
  **Persistent engage order / chain kills (implemented):** `FUN_0041E2D4`
  (called from `FUN_0041ECC8` while `Entity+0x02 & 0x01` is set and the
  approach bit `0x04` is clear) re-acquires after every kill instead of
  stopping. When the current target is dead (action row `3`/`9`) or gone it
  calls `FUN_0041FB64(own group)` for the nearest hostile group around the
  party leader - same location, not the party group, class != `0x53`, and
  either an object/monster group (`class & 0x80 == 0`) or a citizen group
  woken to activity `1`; for the party group (`class & 0x20`, `Game.N` group 0
  = `0x20`) the search area is the leader +/- `(r + r/2)`, `r = 20/9` at
  800x600 (`_DAT_0081D6E0`/`_DAT_0081D6A4`) -> **+/-30 x, +/-13 y** - then
  `FUN_00420190` (nearest member no engaged party unit already targets) with
  the `FUN_00420098` plain-nearest fallback. No candidate clears the order
  (`+0x02 &= 0xFE`). Archer orders (`0x44`) re-pick through `FUN_00410E84`
  (first visible member of the nearest hostile group with a clear
  `FUN_0041130C` line of fire). Implemented as `app/combat/acquire.rs`,
  `ActorMotion::engaged` (serde-skipped: the remake save has no order byte,
  load derives it from a live `combat_target`) and `PlayableApp::player_engaged`;
  right-click attack orders set the same bit, and `SPACE`-to-peace (`& 0xFA`)
  and `F` halt (`0x60`) clear it. This is why a recruited companion ordered
  onto an ant group keeps killing the next ant after each kill.
  **Tick rate (verified):** the original frame budget is **60 ms** (`konung.res`
  `0x42b32f` `MOV [0x81d5c8],0x3C`; limiter `0x42b61a`; `timeGetTime`), so the
  game advances one animation/logic frame per 60 ms (~16.67 Hz; the periodic
  population pass keys off `_DAT_0081d640 & 0xF` ≈0.96 s). `actors.rs`
  `LOGIC_TICK_HZ = 1000/ORIGINAL_FRAME_MS` drives cell stepping; the protagonist
  sprite is synced to the same clock (`app.rs` `player_frame_index`) so one walk
  cycle == one cell (≈0.72 s/cell for 12-frame walks, not the old 60 Hz/0.2 s).
  **Step commit timing (verified):** each cell step is committed at the START of
  its walk segment by the tail of `FUN_0041435C` (action `1`/`7`, queue head
  `+0x13 != 0xFF`), which first stores the pre-step projection into `+0xE8/+0xF0`.
  So while walking the stored cell `+0x08/+0x0A` already **leads** the drawn
  sprite (drawn from the pre-step cell via the `+0xE0` frame dx/dy anchor) by one
  step. A mid-walk move command (the click handler `FUN_0042260C` stores the
  destination inline; the actor logic `FUN_0041ECC8` → `FUN_0043DA1F` →
  `FUN_0041435C`, invoked from the per-frame group dispatcher `FUN_00421B50`)
  therefore re-plans from the cell being walked **into** — never
  from the cell being left (`app.rs` `player_render_x/y`, `player_active`;
  `actors.rs` `ActorMotion.render_cell`/`walking`).
- Mouse cursor: the original hides the Win32 cursor (`ShowCursor(0)` `0x42B574`;
  `SetCursor(0)` in WM_SETCURSOR `0x42B8E0`) and draws its own 32×32 cursor. The
  11 frames are the first `GRAPH.RES` blocks at `0x20000` (`FUN_004392B4` →
  table `0x81A83C`; `res/graph.rs` already steps over them). Format
  `[u32][u32 size]` + a **16-bit color** sprite `[u16 w][u16 h][h×u16 row-size]
  [rows]` (`FUN_0043D2FE`: `0x00` = row end, `b & 0x80` = skip `b & 0x7F` px,
  else `b` literal 2-byte px — no palette). `FUN_00428740` selects the frame:
  base `_DAT_0081D660` (`FUN_00424E0C` class map `0,2→10; 1,12→2; 3..5→0; 9→8;
  else 9`, reset to `4` by `FUN_00428E98`) while an item is in hand, otherwise
  inside the world viewport (`_81D738/_744/_754/_760`): interactive prop +
  settlement + a selected ranged-ready unit → `6`; own party/leader → `4`
  (Ctrl `_81D5E4` → `5`); any other actor → `1` if any selected unit is in
  battle stance, else `3` for non-talkable/Shift (`_81D624`)/leader-in-battle,
  else `5`; free ground cell flagged `0x2000` (active ground WorldObject) → `7`;
  occupied cell → `3`; other free ground → `4`. The `0x2000` flag is ORed for
  every active ground marker by the map loader `FUN_0043ADF0` and cleared by
  `FUN_0041093C`; corpse loot sets it in `FUN_00418480`. Blitted at
  `_DAT_0081D5C0/_D5C4` by `FUN_0042A470`. Ported: `res::cursor` (decode all
  11), `app::cursor::{select_cursor,cursor_index}` (pure precedence + geometry),
  drawn by `draw_detail`; the browser hides the OS cursor in game mode
  (`app.js`) and syncs Shift/Ctrl via `set_shift`/`set_ctrl`. `tools/cursor_png.mjs`
  dumps all 11 (`debug_cursor_count` / `debug_decode_cursor_image`); the feedback
  smoke probes the hand with `debug_world_object_count`/`debug_world_object_cell`/
  `debug_cell_screen`.
- Class-11 items (`ItemInstance +0x00 = 0x0B`) are special/quest/usable items
  dispatched by `FUN_00414DE4` on the subtype byte. `app/class11.rs` is the
  dispatcher: C2 implements the skill books (1–5), strength (8), identify (9),
  XP (10), wine (12), birch letter (28), torch (31), charisma (36/39) and the
  relics state byte (38); C3 implements the duplicator (7), the sword parts
  (22/23) and the amulet combinations (25–27/41–43), including held-item-on-
  target routing via `try_craft_held_at`; C4 implements resurrection (6), the
  magnet (13), the orbs/keys teleports (14–21/44/52/53), the skeleton lamp
  (30), whistle/pipe scatter (35/37) and the mirror (48); D2 implements the
  doll (33: `Entity+0xD2 = 0x13` on every living actor of every other group in
  the current location, consumable). Subtype 11 («Сын Луны») has no switch case
  and is **never** consumed: it is a passive artefact whose class-11 template
  weight is `0xD8F0` = `i16` −10000, so carrying it subtracts 10 kg from
  `FUN_0041B7E4`. Weight is therefore signed end-to-end (`ItemInfo.weight:
  i32`, `item_weight`/`carried_weight`/`capacity: i32`). Subtype 24 («Лопата»)
  arms the dig cursor (`_DAT_0081D660 = 7`, `dig_armed`) and is **not**
  consumed; carrying it in the 32 inventory slots is what allows buried
  WorldObject records (negative `+0x10` money) to be collected on arrival
  (`FUN_00421B50` → `FUN_00433A8C(0x19)`). Buried records stay in the active
  marker pool but are never drawn (`FUN_0042532C`), give no ground cursor
  (`FUN_00428740`) and pay `abs(money)`; the copper mirror flips them for the
  current location. Full trace + the 92-spot table: `DIG_SYSTEM.md`. Full
  subtype catalogue and package status: `CLASS11_ITEMS.md`.
- Items: `item.rs` decodes the **loaded save file's** (`Game.0/1/2`) 16-byte item
  instances at offset 0 (`+0x00` class, `+0x03` subtype; 8192 × 16, read by the
  original loader `FUN_0043a870`) and the `konung.res` class→table→subtype
  templates. The original selector `FUN_0042ba40` picks the icon/weight table by
  class (icon = `u16@+0x02`, weight = `i16@+0x00` — **signed**, `FUN_0041B7E4`
  sign-extends it), while the CP866 **name** is a
  parallel table with a *different* base read at `+0` by `FUN_0042f5f8`
  (class `0..=5|12`: icon/weight `0x459B88` / name `0x459B50` stride 64;
  `6..=8`: `0x45812A` / `0x4580B0`; `9`: `0x4594CA` / `0x459450`;
  `10..=11`: `0x460996` / `0x46091C`; the last three stride 128).
  `Game.res` is **not** the instance table for a loaded game (it only supplies the
  variant table); `Res::item_info` resolves against the attached `Game.N`
  (`Res::set_save_game`). `PlayerCharacter` carries
  `equipment[6]`/`inventory[32]`/`additional[5]`/`special`, attached once from the
  starting `Entity[0]` (`attach_starting_items`). Dragomir (`Game.1`) starts with
  `Нож` (id 1, equipped in slot 2), `Правая часть амулета Дракона` (id 2) and
  5× `Лечебный бальзам` (ids 3..7). **No armor or shield** is equipped at start: equipment slots 3..=5 are empty (base armor `+0x4E = 10` is innate). Equipped items also drive the **actor
  overlays**: `ItemInfo::equipped_visual` (prototype `+0x3E`, class `0..=5|12`)
  selects one of the 24 GRAPH `human_overlay_positions`, composited by
  `res.rs::compose_human` using the original `FUN_00427e9c` rules:
  `HUMAN_DRAW_ORDER_PEACE[facing]` in peace, `HUMAN_DRAW_ORDER_BATTLE_A/B`
  (`[action_bucket][facing]`) in battle (`Entity+0x03 & 0x04`), and the
  `equipped_visual + 1` "in hand" variant for `Entity+0x12` (active weapon slot)
  / slot 5. `actor_frame_state` (NPCs) and `app.rs::player_frame` (protagonist)
  pass the actor's `Entity+0x12`, stance and `Entity+0x66` /
  `PlayerCharacter.equipment` (monsters get no overlay). The character sheet
  also renders the original 160×160 paper doll via `Res::paper_doll_frame`
  (`FUN_0042ea6c`; body origin (0x3C,0x14), per-resolution position table
  `0x46035C`/`0x460360`: 640 → (470,127), 800 → (570,160)) using the same
  overlay plan.
- Melee combat / death / corpse loot (**implemented**): `combat.rs` holds the
  verified rules (hit `rand()%101` vs reaction; monster / armed / unarmed damage
  `STR*HP/400` / `STR*HP/400` / `STR*HP/800`; weapon modifier table `0x462798`
  selected by item
  instance `+0x02` vs target `Entity+0x04`; armor/endurance mitigation
  `round(16*(1+(incoming*0.3+max(0,incoming*0.7-armor))/endurance))`; facing
  dodge `0x450DB4`; armor/weapon durability scales `0.001`/`0.0001`). Right-click
  an actor issues order `0x44/0x41` only to selected units already in battle
  stance (`Entity+0x03 & 4`), never a move. Melee resolves on the last
  attack-animation frame; death rows `3`/`9` hold their final frame. Every 16
  logic frames `hp -= periodic_damage`. Corpse markers reproduce
  `FUN_00418480`: money + up to 22 inventory ids + a class-9 subtype-0
  «Ядовитое жало» when `Entity+0x52 != 0` (poison in instance `+0x0C`);
  generated items only on non-zero killer level deaths (15% roll), not melee.
  Runtime: `app/combat.rs`; loot: `combat/loot.rs`; tests: `combat/tests.rs`;
  headless check: `tools/combat_smoke.mjs`. **Ranged combat implemented:**
  action row `4` spawns an arrow on frame `count-6`; bow = `Equipment[1]`,
  ammo = `Entity+0xCC` (class 12, `+0x04` quantity, decremented per shot, zero
  clears the slot); bow durability `-0.1` per shot and breaks at zero. Damage =
  `(bow modifier vs target archetype or bow rating) * bow rating`, then the
  same armor/endurance mitigation; dodge `rand()%101 <= dex_base*0.5`.
  Projectiles use the original 100-entry pool, 8 sub-steps/frame, INTR sprites
  `184 + 2*facing`, and stop on blocked terrain/actors (friendly fire). A ranged
  kill also gives `power/4` money + `power` archery mastery and passes `power`
  as corpse-loot bonus. Right-clicking a prop targets it only with plain ammo
  (`+0x02 == 0`); a blocked-cell pixel hit triggers settlement target state
  `+1=3`/`+6=1` (21%), `+0x4B2 |= 0x15`, and wakes settlement group `+0x02`.
  Fired arrows are **not** recoverable; lootable arrows are the victim's own
  inventory stacks (e.g. Game.1 entity 40 carries 30 «Кремниевые стрелы»).
  Runtime: `app/combat.rs` + `combat/projectile.rs`; smoke:
  `tools/ranged_smoke.mjs`.
- Save/load (**implemented, browser-native**): `save.rs` (versioned `OPKS`
  payload: bincode + deflate) and `app/save.rs` (capture/apply) persist the
  original snapshot state classes without a filesystem: full **300 × u32 B7
  quest-flag slots** (including `+0x02` journal lines), item-instance table,
  Group[200], Entity[2000], Settlement[20] + merchant stocks, WorldObject
  sources, active ground markers, per-map population/motion, party, discovered
  world-map flags, world tick, projectiles and the combat RNG. `abi/save_api.rs`
  exposes `save_snapshot`/`load_snapshot`; `www/app.js` stores each slot as a
  compressed base64 payload in `localStorage` (`openkonung.save.<slot>`, names in
  `openkonung.sav`). The original `SAVE{width}R.RES` screens are hit-tested with
  the exact `0x46241C` 10-slot rectangles and show the stored names. Loading
  restores population/markers directly (original load mode 1), never re-placing
  `(0,0)` actors or rebuilding WorldObjects.
- Ground WorldObjects / collectable herbs (**implemented, saved**):
  `Game.N +0x21900` is `500 × 0x40` (`FUN_0043A870` reads it to `0x77DEB4`).
  Layout: `+0x00/+0x04` activation timestamps, `+0x08/+0x0A` runtime cell,
  `+0x0C` sprite id, `+0x0E` source index, `+0x10` signed money, `+0x12`
  location, `+0x14..+0x3F` 22 item-instance ids. `FUN_0043ADF0` filters by
  location and the activation test (`+0x00 >= 0`, or `+0x00 != +0x04`, or
  `now + +0x00 >= 0x1518`), and `FUN_00438CC8` resolves the ground sprite:
  exactly one item + no money → that item's world sprite, else `0xA3`.
  `FUN_0042532C` draws the marker pass after terrain/entities/party rings, in
  x-then-y order, at `FUN_00438A78` (Entity projection) minus half the sprite
  size, **only when `+0x10 >= 0`**; negative-money records are buried (the
  shovel/dig system, see `DIG_SYSTEM.md`) and stay in the active pool but are
  never drawn and give no ground cursor. Pickup is **not** walk-over: a
  right-click sets `Entity+0x03` bit
  `0x40`, and on arrival `FUN_00421B50` calls `FUN_004110EC`, which adds money
  then each item (`(endurance_base*1000)/3 + 10000` weight limit, 32 inventory
  slots, insert at slot 0, no stacking); a buried record is skipped unless
  `FUN_00433A8C(0x19)` finds the shovel (class `0x0B` subtype `0x18`) in the
  leader's 32 inventory slots, and pays `abs(+0x10)`. `FUN_0041093C` respawns only a sole
  class-9 subtype-1/2 item: it clones the collected item into a free instance
  slot, writes the new id to the source record and stores `-frame` in
  `+0x00/+0x04`; every other emptied record gets `+0x12 = 0` (permanent).
  `Белый корень` = class 9 subtype 2, world sprite INTR `168`;
  `Земляной орех` = class 9 subtype 1, world sprite INTR `160`. Game.1 has 95
  herb nodes (loc 3: 19 nuts; loc 4: 1 nut + 4 roots; loc 27: 5 roots). Ported
  in `world_objects.rs` (parser/activation) + `app/world_items.rs` (rebuild,
  pickup, respawn) and `app/world_draw.rs` (marker pass). Respawn is evaluated
  on map (re-)entry; WorldObject sources, active markers and item instances are
  captured by the native save payload. `item.rs` now exposes
  `ItemInfo::world_sprite`.
- Dialogue/quest: all dialogue, conditions and actions live in the separate
  `QUEST_R.RES` (337800 bytes, seven fixed blocks; `konung.res` is code only).
  Loaded verbatim by `FUN_0043a870` (`0x43A870`). A **right-click**
  (`FUN_0042260C` right-button branch, `_DAT_0081D5B0 & 2`, command code 0) on an
  NPC whose `Entity+0xD0` (`dialogue_id`) != `0xFF` and which is human
  (`archetype & 0x40 == 0`) calls the dialogue opener `FUN_00435290` (`0x435290`);
  the id indexes block B2 (`0x5E2190`, `u16[300]`) for the first node, and the NPC
  archetype (`Entity+0x04`) selects the window template id (`0x0C..0x13`). The
  left button (`& 1`) only issues UI commands and moves the party — it never opens
  a dialogue. Full pipeline in `MOUSE_INPUT_SYSTEM.md` (mouse) and
  `KEY_INPUT_SYSTEM.md` (keyboard).
  `quest.rs` decodes it (`QuestData::parse`, `dialogue_start`, `node`/`option`,
  `entries`, `line_text` = B6->B3 CP866 text, `condition_program`/
  `action_program`, `flag_byte`); ABI `KIND_QUEST = 13`, manifest field `quest`,
  full trace in `DIALOG_SYSTEM.md`. **Implemented (`dialog.rs`):** a
  **right-click** that hits a **human** actor (`archetype & 0x40 == 0`) with
  `dialogue_id != 0xFF` opens the dialogue (left-click only moves); the page's
  `conditional`(f7) entries are the NPC statement and the `unconditional`(f6)
  entries are the selectable options (`-2`/`-1` links end it); hover highlights,
  left-click selects/follows `+0x04`. It
  renders the window (tiled `INTR[5]` 100x100 stone background via `FUN_00430430`,
  then `INTR[1]` border + `INTR[0]` divider at the decoded per-resolution
  positions, `FUN_0042CBD0` case 1) with the original small font (`0x58BC34`) and
  the highlight font (`0x58B8F0`) for the selected option (line advance 15 px via
  `FUN_0042BD8C`). A right-click on any other actor (monster / non-talkable
  human) falls through to a move; the original instead issues an attack/order to
  the party there (unimplemented). Escape closes.
  Condition/action programs are **executed** by `script.rs` (framework) +
  `app/dialogue_exec.rs` + `app/dialogue_ops.rs`: the `B4` condition evaluator
  (`FUN_00434FE8`, an AND of group-start terms `0x4000`, `0x8000` negates,
  `0x2000` = opcode predicate) gates the f7 statement lines and the f6 options
  and resolves branch pages (`FUN_00434C74`); the `B5` action executor
  (`FUN_00434BB8`, `0x8000` = set flag, `0x2000` = opcode) runs a passing
  statement's action on page build and the selected option's action before
  following its `+0x04`. **Every opcode is dispatched**: low families
  `0x00..0x111` (`script.rs::low_family`) and high `0x112..0x16E`
  (`QuestContext::exec_op`); `0x16A` is implemented as party gear durability repair/cap
  (`npc.smithing*10`, 30% excess reduction). `Both`-kind opcodes run during `B4` as in the original; the page
  builds an owned `QuestContext` snapshot and applies it back once. Unknown/
  unsupported words return `false` rather than inventing behavior. Full
  catalogue in `script.rs` (**trade/loot = `0x113`**, `FUN_00431E2C`); reference
  in `QUEST_OPCODES.md`. Runtime-only quest additions (player `+0xD0/+0xD2/+0xD3`,
  NPC `Entity+0xD2`, settlement money pool/target base, apprentice training
  timestamps, group activity) are persisted in `save.rs::SaveExtra` (save v5;
  v1–v4 decode with defaults). `0x112` sets `Group+0x1D = 1` (activate the NPC's
  group), while
  `0x150` is a pure apprentice-availability predicate and `0x162` only marks the
  apprentice (role bits + training timestamp); the party join is `0x123` later.
  Opening a dialogue queues the NPC voice id (`0x0C..0x13`).
  **Apprentice/station flow (implemented):** `0x162` (`FUN_00434364`) marks the
  first settlement member with `visual < 3`, no role bits, visible, `kind != 3/9`
  and no owned home waypoint, adding the dialogue NPC's producer role bits
  (`0x08` healer / `0x10` merchant / `0x18` smith). The citizen AI
  (`FUN_004168A4`) then walks that member to `waypoint[role]` and stops the
  wander: if already on one of the 8 neighbours (`FUN_0041165C`) it faces the
  waypoint facing, otherwise it moves to the first terrain-free neighbour
  (`FUN_00410FFC`) — never onto the master's own cell. `0x123`
  (`FUN_00431820`) applies the elapsed-time profession gain when the apprentice
  later joins: role 1 healing, role 2 trade, role 3 smithing, with
  `gain = (elapsed/16)/0x5A0` and the `0x4505C8` factor `0.03` (`FUN_0043eaae`
  is a plain round); role 2 is capped by the merchant and 100, role 1 by 500.
  **Barracks training (implemented, `app/settlement_training.rs`):**
  `FUN_00417F84` runs after the producer passes when `waypoint[4].owner` (the
  voivode, `Settlement+0x3F2`, quest predicate `0x130`, installed by `0x159`)
  is non-zero. It decrements `Settlement+0x0C` (`SettlementTrade::timer3`) and
  at `< 1` resets it to `0x2D0` (≈11.5 min, first pass immediate). Every visible
  group member with `visual < 3` except the voivode gets: level +1 when
  `FUN_00410B08(ent, 0)` (class minima) allows, each stored stat
  (`+0x56/+0x5A/+0x60/+0x64`) below the voivode raised by one through the
  `0x45AFB0` cost table paid from a temporary `+1000` XP grant, level +1 again,
  then melee/archery mastery `+= (voivode - own)/deficit + 1` and the
  `FUN_0041C69C` no-effect recalc. See `SETTLEMENT_TRAINING.md`.
- Trading (**implemented**, `trade.rs` + `trade_screen.rs` + `app/trade.rs`):
  dialogue `0x113` (`FUN_00431E2C`) opens interface mode 7. Four 32-slot rows
  (`0x81CB24`): player bag/offer, partner stock/offer. Cell clicks move the
  item between paired rows, arrows page each row (five visible), `INTR[180]` is
  cancel and `INTR[182]` confirm. `FUN_00430D6C` auto-balances money on every
  redraw using `FUN_0041AE00` (sell) / `FUN_0041B5A8` (buy); this implements
  buy, sell, equal exchange and exchange-with-additional-pay. `FUN_004203F4`
  validates 32-slot/weight/money limits and applies the deal: normal NPCs keep
  the sold items, settlement merchants remove bought items from `+0x412`
  (role 2, 24), `+0x444` (role 3, 32) or `+0x484` (role 4, 23) and **destroy**
  sold instances (`class = 0xFF`). Trade skill rises by
  `(table_money + sell_value) / 1024`. `app/settlement_trade.rs` restocks role 2
  (`FUN_004175FC`, herbs/potions) and role 4 (`FUN_004179C8`, equipment,
  including the full-stock branch that moves stock onto settlement group
  members and destroys lower-subtype gear). Role 3 has no producer: the
  per-frame `FUN_0041D048` calls only roles 2/4/5, so it is faithfully
  buy-only. Full trace: `TRADE_SYSTEM.md`; tests `trade/tests.rs`,
  `app/tests/trade.rs`; headless `tools/trade_smoke.mjs`.
- Party system (**implemented**, `app/dialogue_exec.rs` + `hud`): the party is the
  original reserved **group 0** (cap 10); `PlayableApp.party` holds recruited
  companions (canonical `Entity` + `ActorMotion`), slot 0 being the protagonist
  `PlayerCharacter`. HUD cells = party slot (`FUN_0042C730`: cell =
  `entity_index - group.first_entity`). The selectable opcodes: **`0x123`
  (`FUN_00431820`)** recruits the dialogue NPC (appends to group 0, `+0x05=0`,
  `+0x02=0x60`, `+0x14=0xFF`), **`0x14C` (`FUN_00432954`)** settles the NPC into
  the **current settlement group** (a party member leaves the party and stays in
  the village; it is not deleted); the
  predicates `0x15E` (`Entity+5==0`, party member — `FUN_00434228`), `0x161`
  (`Entity+4 & 0x38`), `0x15D` (`max_party <= count` — `FUN_00434168` +
  `FUN_00411078`), `0x160` (level>3), `0x138` (settlement present) drive the
  member-vs-non-member dialogue branches. The HUD panel draws every member
  (portrait + selection marker `INTR[173]` + HP/poison bar); clicking a cell
  selects/deselects (`FUN_00424D8C` selection array `0x81A814`, Shift additive);
  a new member is auto-selected. A recruited member is **removed from the world
  group in behaviour** (`0x123` appends it to group 0; the remake keeps the
  source `Entity` row as a party overlay, filters it from the location
  population and subtracts `party_ids` from settlement group scans, rather than
  compacting canonical source ids) and **follows the leader** (`app/party.rs`:
  `FUN_00421b50` group-0 inline AI -> `FUN_0041ecc8` -> `FUN_0041f3c8`; it
  re-targets a free cell beside the leader only when `FUN_0043881c` distance
  `>= 10`, one cell per walk cycle, `LOGIC_TICK_HZ`). A **world click** commands
  every selected member to the same clicked cell (original left-click command 0
  in `FUN_0042260C` writes `Entity+0x0C/+0x0E` on every unit in `0x81A814`), so
  companions walk with the leader immediately; the 10-cell follow AI is only the
  straggler backup. Companions render in the
  world depth pass, remain right-clickable (party dialogue), and snap to the
  leader on a map transition; dismissing (`0x14C`) settles the NPC into the
  village group at a free slot (the population is rebuilt). Proven end-to-end by
  `app::tests::sigma_joins_the_party_through_dialogue` (Сигмар, `Game.1` entity
  539, `dialogue_id 97`: page 1771 → 1774 → `0x123`; re-talk page 1781 → `0x14C`).
- Quest file selection: the executable opens the literal name `QUEST_R.RES`. The
  install ships it as `Quest_r.res` (= `QUEST_R.RES`, case-insensitive alias) and
  a **different** placeholder `QUESTR.RES` (its dialogues 97/96 are literal `"???"`
  stubs); `make_manifest.py` prefers `QUEST_R.RES` then `Quest_r.res` then
  `QUESTR.RES` and records the **actual on-disk spelling** (`Quest_r.res`) in the
  manifest/pack, so the real script is loaded and the case-sensitive fallback
  URL resolves (the old order could pick the placeholder, and the old manifest
  wrote `QUEST_R.RES`, which 404s on Linux).
- Sound: `sound.rs` decodes `SOUNDS.RES`/`Sounds_.res` (8-byte header + `1000 x
  {u32 off,u32 size}` index, data base file offset 8008, raw s16 mono PCM; 391
  clips in `SOUNDS.RES`). The original uses DirectSound with three buffers at
  22050/19050/25050 Hz (`FUN_00429C0C`) and plays clips via `FUN_0042A13C`;
  `PlayableApp.sounds`/`Frontend.sounds` queue ids for exactly one frame (cleared
  at the start of `step`, drained by the shell before the next step). ABI
  `sound_count`/`sound_id` follows the active frontend/game; `www/app.js` plays
  them via WebAudio. The shell creates the AudioContext at boot (suspended) and
  resumes it on every pointer/key gesture and playback attempt; without that
  resume the context stays suspended forever and no effect is heard. `sound/events.rs` holds the verified 22-call-site mapping:
  UI `4`, item use `9`, map item `10`, equip `11`/`1`/`0`, dodge `6`, human
  action `34/35/36/7/3/2/8`, monster bank `arch*8-0x1D0`, dialogue voice
  `0x0C..0x13`, ambience `0x100+(loc-1)*8`. `KIND_SOUNDS = 14`, manifest field
  `sounds`; report `SOUND_SYSTEM.md`. Positional pan/volume and 19050/25050
  pitch are deferred.
- Frontend: `menu_res.rs` (native menu-image RES decoder), `font.rs` (original
  proportional bitmap fonts embedded in `konung.res`, small 0x171E34 / large
  0x16193C), `frontend/` (original `MENU`/`NEWH`/`SAVE` screens). The New-Hero
  screen follows `FUN_0042EA6C`: tile `NEWH#1` (100x100 stone) behind the
  transparent `NEWH#0`, draw numeric values at regions 7-14/24 and the image
  `NEWH#2` marker in the class box and the skill boxes 15-19 (combat: `archery <
  melee` -> 15 else 16; civil: `identification > 10` -> 17, `trade > 10` -> 18,
  `healing > 50` -> 19), draw the 160x160 paper-doll preview at region 22
  (`(470,127)` @640 / `(570,160)` @800), and draw the hovered/clicked control's
  description from the 20-record CP866 table at `0x46331C` into the big panel
  (region 23) via `FUN_0042F578`/`FUN_0042BD8C` (15 px wrap). Bottom controls:
  **region 20 (left) = Ok** (`NEWH#11/#12`), **region 21 (right) = Отмена**
  (`NEWH#9/#10`). Entry defaults mirror the original init: Dragomir + Warrior,
  Fencing + Identification, stats 3/3/15/10. Hover does not highlight portraits/
  buttons; only clicks select. No always-on debug borders (the `K` overlay still
  outlines every hitbox).
- Character: `character.rs` separates hero identity (`hero.rs`) from
  `CharacterClass` (`Entity+0x04` + stat preset) and holds the one authoritative
  `PlayerCharacter` in `PlayableApp.player`. `FrontendCommand::StartGame` carries
  the final `CharacterBuild` (class, skills, primary stats, `available_xp`),
  copied once at start (never regenerated). The New-Hero screen's stat steppers
  (`NEWH` regions 25-32) spend/refund `available_xp` via the original
  `stat_cost` tiers; `max_load` (`Entity+0x62`) and `max_party` (`Entity+0x58`)
  are derived, live-rendered, and not stored. The seven secondary skills are
  driven by two grouped selectors — regions 15/16 = combat
  (Fencing/Marksmanship → melee/archery/building/smithing) and 17/18/19 = civil
  (Identification/Trade/Healing → trade/healing/identification); regions 13/14
  (Smithing/Building) are original no-op handlers. These values are persistent data only —
  they do not affect gameplay yet. The `Runtime` in `lib.rs` owns **both** the frontend and the live
  `PlayableApp` at once: Escape opens the menu without discarding the game, and
  `Resume` reactivates it. `MENU #1` is the Settings screen; `SAVE #0/#1` are
  the Load/Save screens.
- Settings / `KONUNG.CFG` (**implemented**): `frontend/settings.rs` snapshots the
  seven live globals on entry, the exact `0x46257C` 10-record hit table
  (`SETTINGS_HIT_640`/`_800`, inclusive x2/y2 converted to `Rect`) toggles
  video 640/800, transparency, shadows, day/night, light effects, music and
  sound, and `MENU#2` markers sit centered in enabled controls. **Ok** persists,
  **Отмена**/Escape restores the entry snapshot. `config.rs` holds the exact
  28-byte layout (`video, transparency, day/night, light effects, music, shadows,
  sound`, little-endian `u32`); `abi/config.rs` exposes `config_load`,
  `config_write`, `config_revision`, and `www/app.js` stores those bytes as
  base64 in `localStorage["openkonung.cfg"]`. `frontend_boot(0)` uses the
  configured video mode; the live game remains fixed 800x600.
- Lighting (**implemented**): `res/lights.rs` decodes `LIGHTS.RES` (`0x1000` 64x
  8x8 masks, `0x0AC0` 64x43 mask, nineteen 114x64 tiles) and `LIGHTS.N`
  (`0x3200` low 160x80 map, `0xC800` high 320x160 map); `resource_pack`/packer/
  manifest/app.js carry `KIND_LIGHTS=16` plus all 40 `LIGHTS.N` maps.
  `lighting.rs` ports `FUN_00428FE0`'s `world_frame % 0x5460` ambient delta
  (thresholds `0,1800,1830,3570,3600,5400,8100,11700,11880,11910,16155,16200,
  18900`) and `app/lighting.rs` shades the finished world, blends low-map light
  tiles back over terrain, and redraws sprites on non-zero high-map cells with
  their unshaded colors. Gated by `light_effects`; `day_night` disables ambient.
- Ground shadows (**implemented**): the original has no dirty-rectangle system;
  it keeps 600 per-scanline run lists (`DAT_004C6B50`). `FUN_0043C040` clips a
  sprite-shaped RLE mask into them and `FUN_0043D247` halves the colour of
  every accumulated pixel — the actor/projectile **shadow** pass, gated by the
  `shadows` config word (`0x81D620`). `render/spans.rs` ports both
  (`add_mask`/`darken`); `res/shadows.rs` resolves the masks. GRAPH frame
  records are `{u32 shadow_offset, u32 sprite_offset}` at
  `human block+0x1728+fid*8` / `object block+0x728+fid*8` (data at `+0x4D28`
  / `+0x3228`), and the 8-byte anchor record's **first** `u16` pair is the
  shadow anchor while its second pair is the sprite anchor. `draw_detail`
  collects the body mask plus one mask per equipped overlay
  (`overlay_plan` order, `0x424ED0`) after terrain/props and darkens before the
  sprite depth blit. Deviations: plain RGBA8 halving instead of the 16-bit
  `(c & 0xF7DF) >> 1`, no run budget, and projectile shadows are not ported
  (INTR sprites carry no dedicated shadow mask). `node tools/shadow_smoke.mjs`.
- Actor gear refresh (**implemented**): `inventory::auto_equip_slot` /
  `refresh_equipment` port `FUN_004104D4`/`FUN_004105A0` — on each AI tick every
  non-party human fills its empty ammo slot (`+0xCC`, class 12) and then slots
  `0,1,3,4,5,2` from the inventory (class == slot, `FUN_004182D4`
  requirement), consuming the entry, and recomputes `Entity+0x12`
  (bow -> 1, slot 0 -> 0, else 2). `app::refresh_npc_equipment` runs it before
  `update_world_ai`. Deviation: object-family actors are skipped because
  `Entity+0x12` is their authored M/O walk delay there.
- Movies (**implemented**): the shipped `Video\*.flm` are RIFF AVI with Indeo
  Video 5 (`IV50`, 640x360, 12 fps) + PCM audio, which browsers cannot decode.
  `tools/transcode_videos.py` converts them to H.264/AAC MP4 with ffmpeg;
  `www/manifest.json` lists the 19 `videos` and the files live in `www/video`
  (fallback) and inside `konung.zip.pack` (pack format 3). `video.rs` +
  `abi/video.rs` queue one movie and its post-state; `www/app.js` plays it in a
  modal `<video>` overlay (click/Esc/Skip ends it), pauses music, and calls
  `video_finish`. Triggers match `FUN_0042ADE4`: startup state 9 =
  `POSTER.FLM`; leader game-over flag = `GAMEOVER.FLM`; settlement defeat
  (`FUN_0041D048`, party/defender health >= 2.0) = `DEFEAT<location>.FLM`;
  WorldObject source index 0 pickup =
  `WIN.FLM`. `credits.flm` is launcher-only. Missing/failed movies fall through
  to the same post-state. `server.py` supports HTTP Range for `<video>`.
- `src.zip` is a **stale snapshot** of `src`; edit the files, not the zip.
- `www/manifest.json`, `www/manifest.raw.json`, `www/openkonung.wasm`,
  `www/konung.zip.pack` and `www/video/*.mp4` are generated build artifacts -
  never hand-edit; rebuild after Rust/video changes.

## Coordinate / collision rules (easy to get wrong)
- **Render mapping for actors/targets/transitions** (`app.rs`): the protagonist,
  NPC/mob sprites, the click target marker, and the transition trigger/arrival
  markers are all drawn with `coords::entity_to_local_px(_f)` (the verified actor
  projection below). The obsolete `transition_xy_to_screen` helper (`x*29`,
  `y*16`) has been **removed**; do not reintroduce a second actor projection.
- Entity/transition `x/y` (`Entity+0x08/+0x0A`) **are** the 320×160 runtime grid:
  the original indexes `collision[x*160 + y]` with them directly
  (`KONUNG_SOURCE_OF_TRUTH.md` §1.1/§3, `FUN_0043DA1F`). The `0x438A78` transform is
  only the grid→pixel projection applied at draw time; it is **not** a grid index.
- **Actor/protagonist projection (verified).** `FUN_00438A78(out_x, out_y, x, y)` =
  `(*out_x = y*58 + (2-(x&1))*29, *out_y = x*16 + 16)`, ported as
  `coords.rs::entity_to_local_px`/`_f`. `FUN_0041435C` calls it with
  `Entity+0x08/+0x0A`, stores the result in the runtime render fields
  `Entity+0xE8/+0xF0`, and derives screen coordinates
  `Entity+0xE4 = camera_x + +0xE8 + frame_dx`,
  `Entity+0xEC = camera_y + +0xF0 + frame_dy` (`konung.res` `0x41435C`). It therefore
  **is** the original actor render projection for the protagonist and NPCs
  (`KONUNG_SOURCE_OF_TRUTH.md` §1.2/§4); the remake draws actors with it
  (`app.rs`, `actors.rs`). This corrects the earlier AGENTS note that called it
  "not the render projection".
- Collision source is Konung.N `+0x06400`, 0xC800 bytes, 320×160 `u8`,
  `index = x*160 + y`; expanded to `u16` where low 12 bits `== 0` means free. It is
  the same runtime/Entity grid, so it is applied to player and NPC movement (route
  search + per-cell passability). Truncated Konung.N now fails boot.
- **Movement / pathfinding (verified, `konung.res`).** A world click handler
  (`FUN_0042260C` command 0) stores the destination **inline** on every selected
  unit (`0x81A814`, `[0] = _DAT_0081D56C`) — `Entity+0x0C/+0x0E` = clicked cell,
  `Entity+0x02` = **order byte** (`0x48 'H'` move, `0x44 'D'`/`0x41 'A'` engage,
  `0x60` halt; bits `0x01` engage / `0x08` move / `0x02` target-lock / `0x04`
  approach-mode / `0x20` AI-on / `0x40` route-pending), `Entity+0x03` = flags
  (`0x04` battle stance, `0x02` bag toggle, `0x40` arrival/occupy, `0x80`
  object-graphics-rebuild-pending — set at map load by `FUN_0043ADF0`, cleared by
  the renderer `FUN_0042532C`), the *action
  row* is `Entity+0x00`, and
  `+0x14 = 0xFF` clears the queue terminator. The route is built on the **next
  per-frame tick** by the group/actor dispatcher `FUN_00421B50` (from
  `FUN_00436214`): group class `&0x40` → `FUN_004123BC`, `&0x80` → `FUN_00415FC8`
  → `FUN_004168A4`, `&0xC0==0` → inline switch → **`FUN_0041ECC8` →
  `FUN_0043DA1F` → `FUN_0041435C`**. The destination+route+wake routine
  `FUN_004142D4` (invoked from the actor logic
  `0x4123BC`/`0x412E00`/`0x414DE4`/`0x4168A4`, **not** from the click handler)
  writes the same fields and then runs the route builder; failure leaves the
  target stored but does **not** start a walk (blocked/unreachable points never
  move). `FUN_0043DA1F` searches the collision grid over the **parity-selected
  8-neighbour set** and writes a **direction-code queue** (0..7, ≤31 entries) to
  `Entity+0x13`. Direction→cell deltas are the tables `0x48010C` (odd x) / `0x480110`
  (even x), ported as `coords.rs::DIR_ODD`/`DIR_EVEN` and applied by
  `coords.rs::step_by_direction` (`FUN_0043D922`). Edge weights: dirs 1/3/5/7 cost 10,
  dirs 0/2/4/6 cost 14. `FUN_004123BC` consumes one cell per completed walk cycle and
  requires `facing (Entity+0x01) == queue head` first, so a straight walk keeps a
  constant facing (the old BFS-cell + `atan2` facing produced the zigzag). Facing a
  target uses `FUN_0043897C` → `coords.rs::direction_from_cells`. The remake ports
  this as `actors.rs::find_path`, `ActorMotion.path: Vec<u8>` and
  `app.rs::player_path: Vec<u8>`.
- World-click UI codes (same `FUN_0042260C` switch, resolved): `_DAT_0081D660` is
  the **base cursor/action id `{0,2,4,7,8,9,10}`** (`FUN_00424E0C` item-class map;
  `FUN_00428740` applies the hover override only when it is `4`) — not a pure
  order state;
  `_DAT_0081D5AC` is the **screen/state id `0..9`** (`0` = in-game). Left-button
  codes: `1`/`101` cycle the active weapon slot `Entity+0x12` (`FUN_004216DC`),
  `2` toggle party battle/peace stance (`FUN_00421554`), `4` party halt
  (`FUN_0042246C`), `5` party battle-engage (`FUN_00421458`), `6` toggle the
  quest-log window (mode 2, `FUN_004242BC`), `7` the character window (mode 3,
  `FUN_00424330`), `8..0xD`/`0xE..0x12` equip the selected item into a
  main/equipment slot (`FUN_0041E450`/`FUN_0041E6BC`). Full table in
  `MOUSE_INPUT_SYSTEM.md` §4.1; the Win32 key→command dispatch (ring, VK table,
  modifiers, gates) is in `KEY_INPUT_SYSTEM.md`.
- Base-terrain records (`src/loc.rs`, 80×160 = 12800 u16) are the *current
  base-terrain arrangement*, not proven runtime geometry. Never rescale/clamp
  transition/entity coordinates to the terrain surface.
- Entity `D6` is a visual index; `D8` is `palette_offset` (palette id = offset/512).
  Renderer family is chosen by `archetype & 0x40`, **not** `kind == 6`.
- Population (`population.rs` → `actors.rs`): `Entity` carries the full authored
  state (`EntityStats`, `equipment[6]`, `inventory[32]`) parsed verbatim from
  `Game.N`; `RuntimeActor { entity, source_group_id, placement }` keeps that one
  canonical Entity and only overrides the runtime *position* for `(0,0)` records
  (authored stats are never derived/regenerated). `Entity::debug_block` /
  `debug_entity_state` dump a source Entity by id. Population **growth**
  (constructor VA `0x420A8C`) is documented but not implemented.
  Citizen movement (`actors.rs` `update_population`, `FUN_004168A4`): the AI is
  selected **by group class** (`Group+0x1E`) in the per-frame dispatcher
  `FUN_00421B50` — only **citizen groups** (`class & 0x80 != 0` and
  `& 0x40 == 0`, e.g. the loc-4 camp group 23 = class `0x81`) run the
  waypoint/wander AI. Object/enemy groups (`class & 0x40`, e.g. the giant-ant
  groups 4/75/76/77/78 = class `0x41`) run `FUN_00412E00`, which **idles in
  place** while the group activity byte `Group+0x1D == 0` (mode-0 map entry and
  `FUN_0041D048` clear it), so **mobs must not wander**. Within a citizen group,
  an actor that **owns** a settlement home waypoint (`FUN_0041182C`:
  `waypoint.owner == Entity index`, first five slots of `Game.N Settlement`,
  `loc.rs`/`population.rs::parse_settlements`) walks to that waypoint and then
  stays idle (stationary, facing the waypoint `facing`); only actors **without**
  an owned waypoint random-wander inside group rectangle A (`FUN_00413A84`,
  using `_DAT_0081D6E0`/`_DAT_0081D6A4` = **20/9** at 800x600, set by
  `FUN_0042781C`). E.g. Гилли (`Game.1` entity 277, loc 4 «Старый лагерь») is a
  waypoint owner and must not wander; his non-owner group-mates still roam.
  Ants stay on their placed cells (idle frame 0).
  **Mob AI parity (Package C):** `app/combat/ai.rs` implements the original
  group-relation pass `FUN_00412C30` every 16 logic frames: for current-location
  `class & 0x40` groups, any party member inside rectangle A sets
  `Group+0x1A = 0`/`Group+0x1D = 1`; otherwise any own member outside the
  rectangle sets `0x20` (return-to-rect), else `0`. `FUN_004245B4` is only the
  wake helper (activity `1`, clear `Entity+0x14` for `flags_a & 1 == 0` and
  `archetype != 0x49`), not a target assignment. `update_world_ai` then assigns
  `ActorMotion.combat_target` with the exact `FUN_00412E00` classes: shared
  `0x41/0x4D/0x4F` (then plain nearest), adjacent `0x44/0x57`,
  nearest-unengaged-then-nearest `0x43/0x45/0x46/0x4C/0x55`, plain nearest for
  `0x47..0x4B`, `0x4E`, `0x50..0x54`, `0x56` and other object archetypes.
  Citizen groups (`FUN_004168A4`) use plain nearest. Armed archers
  (`Entity+0x12 == 1`, melee weapon in slot 0 and ammo in `+0xCC`) are gated by
  the `FUN_0041130C` line-of-fire probe (`combat/los.rs`, port of
  `FUN_00410E84`): every candidate must be reachable by the unit-step DDA over
  the collision grid (`8 * u16@0x459B84 + subtype*0x40` samples, start nudge
  from `0x45ACD0[dir]`, aim `0x1E` px above the target projection, walls
  `0xC000` and occupants `!= 0/0xFFF` block, `< 3` cells returns `-1`). The
  relation pass reads the
  live `ActorMotion.cell` (not the authored Entity x/y) and skips hidden/corpse
  members, matching `FUN_00412C30`. `Group+0x1A` is parsed from `Game.N` and
  rehydrated on save load; activity is already in `SaveExtra`. `0x20` only
  returns members to rectangle A (invalid authored rectangles are skipped).
  Role-station waypoints (archetype `0x38` → waypoint slot 1..3) are handled
  before the home waypoint, so a `0x162` apprentice walks to the master's
  workstation. Citizen groups keep their existing wander
  behavior; an actor with a live `combat_target` is skipped by
  `update_population` so its attack frame is not reset. M/O walking delays
  (`archetype 0x4D/0x4F`) use `Entity+0x12` with the authored `Entity+0xD7`
  ceiling (`Res::entity_raw_d7`), applied at each walk-cycle boundary.
  `update_world_combat` now resolves `CombatTarget::World` cells/death state, so
  autonomous melee/ranged world actors and world-vs-world targets work.

## Package F: growth, character screen, audio, legacy saves (implemented)
- **Population growth** (`src/actors/growth.rs`): `FUN_00420A8C` runs only from
  `FUN_0043ADF0(mode 0)` (map entry), not per frame. `grow_map` walks groups
  `1..200`; the first eligible class-`0x41..0x50` group of the location is
  skipped (original `local_2c` quirk) and every later group grows while signed
  `Group+0x02 < 0` and `+0x1C < +0x18`, adding `0x45E216 + class*4` high word
  per member and clamping to `0`. `FUN_0041BD3C` (16-frame pass) raises `+0x18`
  via the `0x45E31E` low-word interval and decrements `+0x02`. New members
  follow the exact average/visual/HP-`0x640`/empty-gear rules; placement happens
  twice (constructor + caller). `growth_timer`/`growth_countdown` persist in
  `GroupExtra` (save v3; v1/v2 still decode).
- **Occupancy exception** (`src/actors/pathfind.rs`): `find_path_with_exception`
  accepts a blocked goal only when `cell & 0x0FFF` is non-zero, not `0xFFF`, and
  equals the ordered target id (`Entity+0x06`).
- **Character screen requirements** (`character::class::STAT_GROWTH_CAPS/_MINS`
  = `0x45B0AB`, 5 classes x 4 stats x 13 levels): `FUN_00410B08` hides a raise
  cost when the level cap is reached and moves `+0x38 → +0x3C` on raise
  (current half +1). The icon-only row 0 is the "level up" control gated by the
  per-level minima (`can_raise_any`); `INTR[171]` is the 10x10 raise icon.
  Clicks use the `0x465B34` anchors (`I`-region approximation documented in
  `hit_test`). `class_from_entity` now maps 2=Merchant, 4=Chief correctly.
- **Positional audio** (`src/sound/position.rs`): pan `round_ties_even(dy*62.5)`,
  volume `dist * -10000 / 1600` (clamped at `-10000`), requests below `-4000`
  dropped; the secondary-buffer rate is 22050 for `id < 0x20` except
  `2,3,6,7,8`, otherwise `rand()%3` over `22050/19050/25050`. `SoundEvent`
  carries id/volume/pan/rate; the ABI exposes `sound_pan/sound_volume/sound_rate`
  and `app.js` applies StereoPanner/Gain/playbackRate.
- **Legacy saves** (`src/legacy_save.rs`): exact `KONUNG.SA0..9` layout
  (`0xB07A4`) parse/write plus `KONUNG.SAV` (500 bytes, 10x50 CP866 names).
  `export_legacy_snapshot` copies the `Game.N` blocks and patches every modelled
  field (the projectile pool has no recovered field map and exports empty);
  `import_legacy_snapshot` applies the blocks in original load mode `1` (no
  growth, no re-placement). Browser keys: `I`/`E` import/export a snapshot,
  `Shift+I`/`Shift+E` import/export the name table.

## Testing / conventions
- Inline tests live in `src/coords.rs` (actor-projection round-trip plus the
  `0x48010C`/`0x480110` direction tables, `step_by_direction`, direction costs and
  `direction_from_cells`) and `src/loc.rs` (collision decode/index/bounds);
  `src/actors.rs` covers `find_path`, blocked-goal rejection and
  constant-direction straight walks.
- Rust doc comments cite `KONUNG_SOURCE_OF_TRUTH.md` section numbers; keep that when
  touching recovered file formats.
- Treat the original data files and `target/` as immutable inputs/artifacts.
- The git repository root is `web/` on disk. Documentation lives in `web/docs/`;
  the only markdown file at the repository root is `README.md`.
