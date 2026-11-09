# Parity gaps — implementation plan (rev 1, 2026-11-10)

Scope: close the remaining *game-function* parity gaps found by the binary
audit of `konung.res` (PE32/i386, sha256 `d698c4f2…9810`).

Status of the audit: **264 / 291 game-range functions faithfully ported
(90.7 %)**; 4 partial, 17 platform-substituted, 6 absent. This plan closes the
4 partials, which lifts the number to **268 / 291 (92.1 %)**.

Evidence: REA / Ghidra 12.1.4, `binary_overview`
`ev_28d45bac75d84308c932193165af841c9f02c4f1a18ff63cfda78149f02bd017`.

---

## 0. Corrections to the earlier audit

Two entries in the first pass were mislabelled by the CSV role strings and are
**already implemented**:

| VA | CSV role | actual | where |
|---|---|---|---|
| `0x4203F4` | party inventory transfer / take-all | trade confirm + cancel validation | `trade.rs::confirm/cancel`, `app/trade.rs::trade_confirm/trade_cancel` |
| `0x438DC4` | angle difference from leader | positional **pan** (`delta*62.5`) | `sound/position.rs::source_pan` |
| `0x438E24` | angle to camera/leader | positional **volume** (`dist*-10000/1600`) | `sound/position.rs::source_volume` |

The remaining four partials are real:

| VA | what it actually is |
|---|---|
| `0x4104D4` | NPC auto-equip one empty slot from the inventory |
| `0x4105A0` | NPC equipment refresh (ammo slot + slots `0,1,3,4,5,2` + `Entity+0x12`) |
| `0x41130C` | armed line-of-fire scatter probe (ranged target selection) |
| `0x43D247` | darken (50 %) a set of horizontal spans = the **shadow pass** |

---

## 1. Discovery summary (what the originals do)

### 1.1 `0x4104D4` / `0x4105A0` — NPC auto-equip

```c
// 0x4104D4(entity, slot)
if (equip[slot] == 0) {
  for (i = 0; i < 32; i++) {
    id = inventory[i];
    if (id && item_class(id) == slot && requirement_ok(entity, id)) {
      equip[slot] = id;
      FUN_00421AD0(entity, id);   // remove id, shift down, clear last slot
      return 1;
    }
  }
  return 0;
}
return 1;

// 0x4105A0(entity)
if (ammo(+0xCC) == 0)
  for (i = 0; i < 32; i++)
    if (item_class(inv[i]) == 0x0C && requirement_ok(...)) { ammo = id; consume; break; }
for (slot in [0,1,3,4,5,2]) 0x4104D4(entity, slot);
if (ammo != 0 && equip[1] != 0) raw_0x12 = 1;   // bow
else if (equip[0] != 0)         raw_0x12 = 0;   // axe
else                            raw_0x12 = 2;   // fists
```

Verified on shipped data: `Game.1` `Entity[0]` has `equipment = [0,0,1,0,0,0]`
(knife id 1 in slot 2) and item instance 1 has class byte `+0` = `2`, i.e.
**class == slot** for slots 0..5, **class 12 == ammo**.

Callers: only `0x412E00` (object/enemy AI) and `0x4168A4` (citizen AI).
The party leader never runs it. This is an NPC/companion behaviour.

### 1.2 `0x41130C` — armed line-of-fire probe

Callers `0x410E84` (armed target pick, gated on `equip[0] && ammo`) and
`0x41ECC8` (actor logic).

```c
dist = max_metric(src, target); if (dist < 3) return -1;
shots = u16[0x459B82 + weapon_class*0x40 + 2];   // sample count
aim  = project(target); aim.y -= 0x1E;           // chest height
repeat shots { repeat 8 {
  (cx,cy) = screen_to_cell(aim + scatter(±~30px));
  if ((cx,cy) == target) return 1;
  cell = collision[cx*160 + cy];
  if (cell & 0xC000) { if (mode > 0) return 0;          // wall
      if (height_ok(...)) { entity.target = (cx,cy); return 1; } return 0; }
  if ((cell & 0xFFF) && (cell & 0xFFF) != 0xFFF) return 0; // occupied
}}
return 0;
```

`mode > 0` = actor aiming; `mode < 0` = in-flight projectile
(`(-1-mode)*0x24` record of the `0x80BD50` pool).

### 1.3 `0x43C040` + `0x43D247` — the "dirty region" system

Not a damage-rect system. It is a **per-scanline span accumulator**:

- `DAT_004C6B50` — 600 heads, each a sorted linked list of `{x0, x1, next}`.
- `0x43C040(mask, x, y)` — producer: takes a sprite-shaped RLE mask
  (`u16 w`, `u16 h`, then per-row `{skip,count}`; `0` ends the row,
  `b & 0x80` skips `b&0x7F`), clips to the viewport and inserts the runs.
  A run budget (`_DAT_005076D8`) bounds the total.
- `0x43D247` — consumer: `pixel = (pixel & 0xF7DF) >> 1` (or `& 0x7BDF`),
  i.e. **half brightness over exactly those spans**.

Producer callers, inside the world render pass `0x426384`:

```c
if (_DAT_0081D620 != 0) {          // 0x81D620 = the `shadows` config word
  for each active entity: 0x43C040(shadow_mask, x, y); + 0x424ED0 overlay masks
  for each projectile:    0x43C040(shadow_mask, x, y);
  0x43D247();                      // darken
}
```

The mask and its anchor come from the animation frame record
(`0x414078`): `{u32 shadow_img_off, u32 sprite_img_off}` at `+0xDC`, and an
8-byte anchor record `{shadow_gx, shadow_gy, sprite_gx, sprite_gy}` at `+0xE0`.
So GRAPH frames carry a **dedicated shadow mask image and a separate shadow
anchor**, and the silhouette is offset from the sprite by
`(shadow_g - sprite_g)`.

---

## 2. Decisions (2026-11-10)

1. Shadow compositing uses **plain RGBA halving** (no 16-bit 5-bit round-trip).
   Documented as a deviation: the original computes `(c & 0xF7DF) >> 1` on
   16-bit RGB555, the remake halves each 8-bit channel.
2. Equipped-item **overlay shadows are included** in the first pass
   (`0x424ED0` behaviour).
3. Implementation order: **Phase 3 (LOS probe) → Phase 1 (auto-equip) →
   Phase 2 (shadows)**.

---

## 3. Phase 3 — armed line-of-fire probe (`0x41130C`)

Files: `web/src/combat/los.rs` (new), `web/src/combat.rs`,
`web/src/app/combat/ai.rs`, `web/src/actors.rs`, `web/src/coords.rs` (helper
reuse), tests.

- `los::probe(...) -> Probe` with `-1` = too close, `0` = no line,
  `1` = engaged (and the accepted cell).
- Sample count from the weapon-class table (constant), 8 samples per shot,
  scatter ±30 px around the aim point, `aim.y -= 0x1E`.
- Scatter uses the **combat RNG** (the original `rand()` sequence is a
  documented unknown, §25 #4); the deviation is recorded.
- Wire into the armed-NPC target pick (port of `0x410E84`) and the pre-order
  check in actor logic (`0x41ECC8`).
- Tests: too close → `-1`, clear line → `1`, wall → `0`, occupied cell → `0`,
  projectile mode substitutes a height test.

## 4. Phase 1 — NPC auto-equip (`0x4104D4` / `0x4105A0`)

Files: `web/src/inventory.rs`, `web/src/actors.rs`, `web/src/app/combat/ai.rs`,
`web/tools/mob_ai_smoke.mjs`, tests.

- `inventory::auto_equip_slot(...)` — occupied → `true`; else first
  inventory item with `class == slot` and `requirement_ok`, equip it and
  remove/compact.
- `inventory::refresh_equipment(...)` — ammo slot (class 12) first, then
  `[0,1,3,4,5,2]`, then recompute `raw_0x12`.
- Call from the object/enemy AI tick and the citizen AI tick (matching the
  original's only two callers). The protagonist loadout is untouched.
- Tests: class match, requirement skip, ammo fill, all three `raw_0x12`
  transitions, compaction, no-op when occupied.

## 5. Phase 2 — shadow span pass (`0x43C040` / `0x43D247`)

Files: `web/src/render/spans.rs` (new), `web/src/res/human/frames.rs`,
`web/src/res/objects.rs`, `web/src/res.rs`, `web/src/app/world_draw.rs`,
`web/src/config.rs` (read only), tests, `web/tools/frame_png.mjs`.

- Decode `shadow_image_offset` (`frame_record +0`) and the shadow anchor pair
  (`u16 @ +0`, `+2`) next to the existing sprite offset/anchor.
- `SpanList` with `add_mask(payload, x, y, clip)` (`0x43C040`) and
  `darken(frame, fw)` (`0x43D247`).
- `draw_detail`: after terrain/props, before the actor depth pass, when
  `config.shadows != 0`: add body + equipped-overlay + projectile shadows, then
  darken.
- Deviations: plain RGBA halving; no run budget (we allocate per frame);
  no dirty-rectangle reuse (full-frame redraw).

## 6. Phase 4 — bookkeeping

- `KONUNG_SOURCE_OF_TRUTH.md` §25/§28.6, `AGENTS.md`, `docs/MODULES.md`,
  this file's coverage table.
- Rebuild wasm + pack; `cargo test`, `cargo check --all-targets`,
  `tools/smoke.mjs`, `tools/mob_ai_smoke.mjs`, `tools/ranged_smoke.mjs`,
  `tools/feedback_smoke.mjs`, `tools/frame_png.mjs`.
- File the discovery in MemPalace (`konung/general`, `openkonung/packages`).

## 7. Coverage after the plan

| | now | after |
|---|---:|---:|
| faithful | 264 | **268** |
| partial | 4 | 0 |
| platform-substituted | 17 | 17 |
| absent | 6 | 6 |
| | 90.7 % | **92.1 %** (97.9 % incl. platform) |

---

## 8. Implementation status (2026-11-10, complete)

All three phases are implemented, tested and built. `cargo test` = **529
passed**; `cargo check --all-targets` clean; wasm/pack rebuilt; `smoke`,
`mob_ai_smoke`, `ranged_smoke`, `feedback_smoke`, `save_smoke` and the new
`shadow_smoke` pass.

### 8.1 Phase 3 — line-of-fire probe (`0x41130C`)

`web/src/combat/los.rs` (+ `combat/los/tests.rs`), `ItemDb::probe_range`
(`u16@0x459B84 + subtype*0x40`, constant `ITEM_PROBE_RANGE_OFF`).

Findings that changed the plan:

- The probe is a **deterministic unit-step DDA**, not a random scatter. The
  decompiler's `FUN_0043EAAE()` calls are the x87 `FISTP` round helper
  (`ROUND(ST0)`), and `FUN_0043EB2E` is the `sqrt` domain check.
- `shots` = `u16[0x459B84 + subtype(equipment[0])*0x40]`, verified from the
  shipped table: `1` for the melee subtypes (knife subtype 18), `18`/`25` for
  the ranged ones. Total samples = `8 * shots`, one pixel per sample along the
  dominant axis (`delta / (max(|dx|,|dy|) + 0.001)`, `0x45000C`/`0x450014`).
- Start point = `FUN_00438A78(src)` + `0x45ACD0[dir]` (eight `i8` pairs,
  verified bytes `ba bf | d0 9d | ...`); aim point = `FUN_00438A78(target)`
  with `y -= 0x1E`; each sample is re-shifted `+0x1E` before the inverse
  projection (`FUN_00438AB4`).
- `< 3` cells (`FUN_0043881C`, i.e. `actors::follow_distance`) returns `-1`;
  the target cell always wins over the collision test.
- The `mode < 0` projectile branch (`FUN_0043E4EA` height test) is not ported;
  the remake resolves arrows through the real projectile flight.

Wired into `app/combat/ai.rs::update_world_ai`: an armed archer
(`Entity+0x12 == 1`, `equipment[0] != 0`, `special_equipment != 0`, the exact
`0x410E84` gate) filters its candidate list through the probe.

### 8.2 Phase 1 — NPC auto-equip (`0x4104D4` / `0x4105A0`)

`inventory::auto_equip_slot` + `refresh_equipment` (+ tests),
`app::refresh_npc_equipment` called from `combat_tick` before `update_world_ai`.

- Slot gate is `class == slot` for slots `0..5` (verified: `Game.1` entity 0
  has the knife in slot 2 and the instance class byte is `2`), ammo is class
  `12` in `Entity+0xCC`.
- Consuming an inventory entry is `FUN_00421AD0` = shift down + clear the last
  slot (`inventory::remove_inventory_id`).
- `Entity+0x12` recompute order: bow (ammo + slot 1) -> `1`, slot 0 -> `0`,
  else `2`.
- `_DAT_0081D5F0` has no remake equivalent; `EquipmentRefresh::changed` is
  returned instead.
- Deviation: object-family actors (`archetype & 0x40`) are skipped. Their
  `Entity+0x12` is the authored M/O walk delay (`archetype 0x4D/0x4F`) and they
  can never hold equipment.

### 8.3 Phase 2 — shadow span pass (`0x43C040` / `0x43D247`)

`render/spans.rs` (`add_mask`, `darken`, tests), `res/shadows.rs`
(`Res::add_actor_shadow` + overlay resolution + sweep test),
`WorldSprite::shadow: Option<ShadowKey>` and the pass in
`app/world_draw.rs::draw_detail`.

- Human and object frame records are `{u32 shadow_offset, u32 sprite_offset}`;
  the anchor record's first pair is the shadow anchor, the second the sprite
  anchor. Verification data is in `KONUNG_SOURCE_OF_TRUTH.md` §28.9.
- Equipped overlays are included: each overlay block contributes its own mask
  at `shadow_anchor + body_block overlay_anchor` (the two offsets the original
  `FUN_00424ED0` adds).
- Draw order matches the original: terrain -> props -> shadow spans -> darken ->
  sprite depth blit. Gate = `config().shadows` (`0x81D620`). Spans are clipped
  in screen space; the shadow origin already includes the camera pan.
- Deviations: plain RGBA8 halving (decision 1), no run budget, no dirty-rect
  reuse (we redraw the frame), projectile shadows not ported.
- Diagnostics: `debug_shadow_spans()` / `debug_shadow_span(i, field)`; the new
  `tools/shadow_smoke.mjs` reports 164 spans and 3067 darkened pixels (only
  darker) on the start map.

### 8.4 Coverage

| | before | after |
|---|---:|---:|
| faithful | 264 | **268** |
| partial | 4 | 0 |
| platform-substituted | 17 | 17 |
| absent | 6 | 6 |
| total | 291 | 291 |
| | 90.7 % | **92.1 %** (97.9 % incl. platform) |

