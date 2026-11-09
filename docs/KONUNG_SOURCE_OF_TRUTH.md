# Konung 1 Remake — Reverse-Engineered Source of Truth

**Revision:** 4 — binary re-audit of `konung.res` (corrections, binary-only
gates, new §28/§29), 2026-10-15  
**Status:** canonical implementation contract for the remake. 

> Revision 4 re-verified every address this document cites directly against
> `konung.res` (PE32/i386, image base `0x400000`) and the `Konung.N`/`Game.N`
> raw bytes. Section-level verdicts, corrections and newly recovered
> binary-only systems are recorded inline and in §28; the VAs are
> `konung.res`-specific (see §29 for `CLIENT.EXE`). 

This file supersedes earlier coordinate/rendering claims in previous revisions of `KONUNG_SOURCE_OF_TRUTH.md`.

The rule is simple: **the original binaries are authoritative; the remake is not**. A visually plausible old implementation is evidence worth preserving, but it is not allowed to override a complete original-code trace. Conversely, a single disassembled arithmetic function is not enough to assign semantics until its callers and consumers are traced.

---

# 0. Evidence levels and implementation discipline

Use these labels literally.

1. **ORIGINAL-CODE VERIFIED** — directly observed in `konung.res` / `CLIENT.EXE` machine code, including the relevant caller/consumer chain.
2. **BINARY-LAYOUT VERIFIED** — offsets, strides and table boundaries close exactly against original files.
3. **DATA-CORRELATED** — values in real saves/resources strongly match the observed code behavior.
4. **REMAKE-REGRESSION VERIFIED** — empirically preserves a known-correct remake appearance/behavior, but is not necessarily original semantics.
5. **WORKING INTERPRETATION** — useful semantic name, not fully proven.
6. **UNKNOWN** — do not invent a meaning.

## 0.1 Mandatory rule for agents

Before changing a subsystem, answer these checks:

```text
CHECK 1: Which coordinate/data domain is this field in?
CHECK 2: What exact original routine consumes it?
CHECK 3: Is there an explicit conversion before use?
CHECK 4: Is the current remake behavior original, compatibility-only, or simply wrong?
CHECK 5: Am I adding a scale/clamp/fallback that does not exist in the original?
```

If a fact is marked UNKNOWN and is not required for the current task, **preserve it and continue**. Do not stop implementation merely to ask how to rename an unrelated unknown field. Ask only when an unresolved point is a true blocker.

## 0.2 Forbidden inference patterns

Do not do any of the following without original-code evidence:

```text
"the array has 80 columns, therefore world X must be 0..79"
"the visible surface ends here, therefore spawn coordinates outside it are invalid"
"this number looks like a sprite id, therefore use it as a sprite id"
"the old renderer divides by two, therefore the file coordinates must be doubled"
"one group produced one visible actor, therefore one Group == one Actor"
```

Do not add silent fallback humans/monsters, center-spawn fallbacks, coordinate clamps, `/2`, `/4`, or arbitrary bounds to make screenshots look plausible.

---

# 1. Correction history: coordinate/render pipeline

Several earlier claims were individually plausible but incomplete. This section exists so they are not resurrected later.

## 1.1 RETRACTED: "Group spawn coordinates → collision coordinates unresolved"

This is now **false**.

The original runtime placement routine around `konung.res:0x411DE4`:

1. obtains the owning Group through `Entity+0x05`;
2. generates `(x,y)` from Group rectangle A;
3. writes the generated values **directly** to `Entity+0x08/+0x0A`;
4. immediately checks collision using byte addressing equivalent to:

```text
collision_base + x * 0x140 + y * 2
```

Therefore:

```text
Group spawn coordinate
    == Entity x/y coordinate
    == collision-grid coordinate
```

for this runtime path.

## 1.2 RETRACTED: "0x438A78 semantic role unresolved"

The transform is now traced into actor rendering.

Original code passes `Entity.x/+0x08` and `Entity.y/+0x0A` to `0x438A78`, stores the outputs in Entity runtime render fields (`+E8`, `+F0` in memory), and the actor renderer later consumes those values with the camera offsets.

Therefore `0x438A78` is the canonical Entity-grid → local-world-pixel transform.

## 1.3 RETRACTED: "old remake x*29,y*16 is the canonical actor projection"

The old remake placed the protagonist plausibly because the **base-terrain lattice was also rendered at half the original spacing**. Two wrong transforms partially cancelled visually.

The old mapping:

```text
local_x = 29 + x*29
local_y = y*16
```

is not the canonical original Entity projection.

## 1.4 Root cause of the visual regression

The remake corrected the actor projection without simultaneously correcting the terrain lattice. The actor moved to original-scale coordinates while the map remained half-scale.

The original uses:

```text
base-terrain record lattice: 116 px horizontally, 32 px vertically, 58 px odd-row offset
runtime/entity lattice:       58 px horizontally, 16 px vertically
```

The relation is intentionally 2:1. The 80×160 base-terrain record count and the 320×160 runtime/collision grid are different resolutions of the same map geometry.

---

# 2. Canonical coordinate domains

Keep these as separate types or explicit names in code.

```text
RuntimeCell / EntityCoord / GroupSpawnCoord
BaseTerrainRecordCoord
GraphPlacementPixel
PropGridCoord
LocalWorldPixel
ScreenPixel
```

Do not pass a pair of integers between these domains without an explicit conversion.

---

# 3. Runtime / Entity / Group-spawn / collision grid

**Status: ORIGINAL-CODE VERIFIED**

Bounds:

```text
x = 0..319   (320 values)
y = 0..159   (160 values)
```

The expanded collision table is `u16` and x-major:

```rust
index = x * 160 + y
```

Byte addressing in the original:

```text
collision_base + x * 0x140 + y * 2
0x140 = 160 * sizeof(u16)
```

A runtime cell is free for the placement path when:

```rust
(collision[x * 160 + y] & 0x0FFF) == 0
```

`x=268` is valid. Any statement that `268` is outside a 320-wide grid is simply an arithmetic error.

---

# 4. Entity coordinate → local-world-pixel projection

**Status: ORIGINAL-CODE VERIFIED**  
Original function: around `konung.res:0x438A78`.

Given integer runtime/entity cell `(x,y)`:

```rust
local_x = y * 58 + (2 - (x & 1)) * 29;
local_y = x * 16 + 16;
```

Equivalent:

```text
x even: local_x = y*58 + 58
x odd:  local_x = y*58 + 29
local_y = x*16 + 16
```

Known exact checks:

```text
Entity (0,0) -> (58,16)
Entity (1,0) -> (29,32)
Entity (2,0) -> (58,48)
Entity (5,7) -> (435,96)
```

## 4.1 Consumer chain

The original calls the transform with:

```text
Entity +08 = x
Entity +0A = y
```

and stores the results in runtime Entity rendering fields. Actor rendering later uses those precomputed pixel coordinates plus camera offsets and sprite-specific offsets.

This is not a heuristic and must not be replaced by `29+x*29, y*16` once the terrain renderer is corrected.

---

# 5. Camera / viewport transform

**Status: ORIGINAL-CODE VERIFIED at conceptual level**

The original keeps local-world-pixel coordinates separate from camera offsets. Render paths for terrain, GRAPH placements, props, and actors all add global camera/view offsets before final draw calls.

Conceptually:

```rust
screen_x = local_world_x + camera_x + sprite_offset_x;
screen_y = local_world_y + camera_y + sprite_offset_y;
```

Relevant original globals used repeatedly by the renderer are around:

```text
0x81D5CC  horizontal camera/render offset
0x81D5D0  vertical camera/render offset
```

The exact higher-level camera-follow policy is not required for population restoration. The important contract is:

- file/grid coordinates are never mutated merely to follow the camera;
- projection produces local-world pixels;
- camera is applied afterward.

## 5.1 Surface-memory rule

The original is a viewport renderer. It does not require constructing a single giant pre-baked image of the whole map.

A remake may cache a full surface if convenient, but **surface size is an implementation choice, not world bounds**. For fidelity and memory/performance, viewport culling is preferred after the coordinate pipeline is stable.

---

# 6. Konung.N exact file layout

**Status: BINARY-LAYOUT VERIFIED**

File size:

```text
0x1F720 bytes
```

Major blocks:

```text
offset    size      structure
0x00000   0x06400   base terrain: 12800 × u16
0x06400   0x0C800   collision source: 320 × 160 × u8
0x12C00   0x03E80   GRAPH placement layer: 1000 × 0x10
0x16A80   0x08CA0   OBJECT/prop placement layer: 1000 × 0x24
------------------------------------------------------
EOF       0x1F720
```

All boundaries close exactly.

---

# 7. Base terrain layer (`Konung.N +0x00000`)

**Status: ORIGINAL-CODE VERIFIED renderer geometry; BINARY-LAYOUT VERIFIED storage**

Storage:

```text
12800 records × u16
160 rows × 80 records per row
```

## 7.1 Each u16 is two independent 1-byte terrain layers

The original terrain renderer reads **both bytes** of each 16-bit record separately.

For each byte:

```text
0     => no tile in that sublayer
n > 0 => draw terrain resource index (n - 1)
```

Therefore a canonical parser should expose something like:

```rust
struct BaseTerrainRecord {
    layer0: u8,
    layer1: u8,
}
```

Do not render only `record & 0xFF`. The high byte is a second drawable terrain layer.

## 7.2 Original terrain lattice

The original render loop around `0x42532C` advances:

```text
column step                = 0x74 = 116 px
row step                   = 0x20 = 32 px
odd-row horizontal offset  = 0x3A = 58 px
```

Ignoring camera offset:

```rust
terrain_local_x = column * 116 + (row & 1) * 58;
terrain_local_y = row * 32;
```

This is the canonical base-terrain-record lattice.

### Important

Do **not** use the old remake lattice:

```text
column*58
row*16
parity*29
```

That is exactly half-scale.

## 7.3 Relationship to runtime grid

The terrain record lattice has twice the step of the Entity/collision half-step lattice:

```text
terrain record step  = 116 × 32
runtime half-step    =  58 × 16
```

This is expected and explains why:

```text
80 × 160 terrain records = 12800 records
320 × 160 collision cells = 51200 cells
```

Do not attempt to force both tables into the same array dimensions.

---

# 8. Collision/passability source (`Konung.N +0x06400`)

**Status: ORIGINAL-CODE VERIFIED**

Storage:

```text
size  = 0xC800 = 51200 bytes
shape = 320 × 160 u8
index = x*160 + y
```

The location loader expands each byte `b` into runtime `u16` collision state:

```rust
if b == 0xFF {
    value = 0x0FFF;
} else if (b & 0x0F) == 0x0F {
    value = ((b as u16) << 8) | 0x0FFF;
} else {
    value = (b as u16) << 8;
}
```

Required decode checks:

```text
0x00 -> 0x0000
0x0F -> 0x0FFF
0x10 -> 0x1000
0x1F -> 0x1FFF
0xFF -> 0x0FFF
```

The low 12 bits are used as passability/occupancy state.

The table is also modified at runtime to incorporate dynamic occupancy/state. A pure Euclidean actor-separation rule is not the original system.

## 8.1 Runtime upper-bit semantics (ORIGINAL-CODE VERIFIED)

`FUN_0043adf0` (Konung.N loader) and the actor setup/step code mark the upper
bits of the runtime `u16`. These are **not** terrain: passability still tests
only `& 0x0FFF`, but the markers carry real gameplay meaning.

```text
bit  0x1000 -> cell lies inside a Transition trigger rectangle
               (set at load for every cell of every transition whose +03
                source location == current location; see §13)
bit  0x2000 -> cell contains an active world-object / portal prop
               (set for each WorldObject at the current location)
```

Actor occupancy is merged into the same word: when an actor occupies a cell the
entity index is OR-ed into the `u16`, and the cell's low 12 bits are cleared
(`&= 0xF000`) when it leaves. This is why a blocked destination in
`FUN_0043da1f` can still be legal: that routine accepts a blocked goal only when
`(cell & 0x0FFF) == Entity+0x06` (the actor's own interaction target), otherwise
the route fails.

A remake that only reads `& 0x0FFF` (as the current `loc.rs`/`actors.rs` do) can
path correctly but silently ignores triggers, portals and actor occupancy. See
§28.

---

# 9. GRAPH placement layer (`Konung.N +0x12C00`)

**Status: ORIGINAL-CODE VERIFIED coordinate consumption; BINARY-LAYOUT VERIFIED storage**

```text
count  = 1000    (storage stride count)
stride = 0x10
```

The runtime loader loop (`FUN_0043adf0`) is bounded by `count < 999`, so at most
**999** records are active; the 1000th slot is never consumed. The sentinel that
ends the scan is "first dword has bit 31 set" (`< 0x80000000` unsigned), not only
the literal `0xFFFFFFFF`.

File-side record, relevant fields:

```text
+00 dword resource/tile index or sentinel 0xFFFFFFFF
+04 dword resource/palette metadata (patched by original loader)
+0C i16/u16 local pixel X anchor
+0E i16/u16 local pixel Y anchor
```

The original loader patches resource pointers/metadata but does **not** run the `+0C/+0E` coordinates through the Entity/grid projection.

The renderer directly uses:

```rust
screen_x = camera_x + graph_x_px;
screen_y = camera_y + graph_y_px;
```

Thus GRAPH placements are already in a **pixel-anchor domain**.

### Implementation rule

Do not treat GRAPH placement X/Y as terrain-record coordinates, runtime cells, or Entity coordinates. No `*58`, `*16`, `/2`, or parity transform is required.

---

# 10. OBJECT/prop placement layer (`Konung.N +0x16A80`)

**Status: ORIGINAL-CODE VERIFIED coordinate conversion and rendering**

```text
count  = 1000    (storage stride count; runtime loop is `count < 999`)
stride = 0x24
```

The original loader reads raw records into memory and preprocesses them in place.

## 10.1 Relevant file-side fields

```text
+00 i32  raw object resource index; negative terminates/marks unused region
+04 dword palette/resource state
...
+1E u16  raw grid Y-like coordinate
+20 u16  raw grid X-like coordinate
+22 u8   flags/state (shares the tail near +20)
+23 u8   animation/runtime byte
```

The loader (`FUN_0043adf0`) uses the raw index **directly** to address the
runtime OBJECTS object record (`raw*0x100 + 0x75FCB4`) and adds `30` (`0x1E`)
only to derive the **entity-type id** used by the 512-entry activity/occupancy
table `0x81B954` (`type = raw + 30`). The remake's `object_id = raw + 30`
therefore names the *type* index; confirm it is also the correct Objects.res
bank/sprite selector (the loader consumes the raw index for the object record
itself) before treating it as the render selector.

## 10.2 Canonical names for raw prop coordinates

Use:

```text
prop_grid_x = raw word +0x20
prop_grid_y = raw word +0x1E
```

Do not name `+1E` X merely because it appears first in memory.

## 10.3 Original prop grid → local-pixel conversion

The loader converts these raw grid coordinates in place:

```rust
prop_local_x = prop_grid_y * 58 - (prop_grid_x & 1) * 29;
prop_local_y = prop_grid_x * 16;
```

It stores the converted pixel anchors back into the runtime copy of the record.

This is **not** the base-terrain `116×32` lattice and is **not** `entity_to_local_px(y,x)`.

The relation to Entity projection is:

```text
entity_local_x(x,y) = prop_local_x(x,y) + 58
entity_local_y(x,y) = prop_local_y(x,y) + 16
```

when the same runtime-grid `(x,y)` is used. This difference is an anchor convention, not a coordinate-domain mismatch.

## 10.4 Prop renderer consumption

The object renderer later takes the preconverted prop pixel anchor, adds camera offsets and frame/object-specific sprite offsets, then draws.

Conceptually:

```rust
screen_x = camera_x + prop_local_x + object_frame_offset_x;
screen_y = camera_y + prop_local_y + object_frame_offset_y;
```

### Answer to the previously-blocking "Prop projection" question

The canonical projection is:

```rust
fn prop_grid_to_local_px(x: i32, y: i32) -> (i32, i32) {
    (y * 58 - (x & 1) * 29, x * 16)
}
```

where `x = raw +0x20`, `y = raw +0x1E`.

Do **not** tie props to the 116×32 terrain-record lattice. Do **not** swap arguments into `entity_to_local_px`.

---

# 11. Unified map geometry summary

This table is the primary "do not ask again" reference.

| Source | Stored coordinate domain | Local-world-pixel transform before camera |
|---|---|---|
| Base terrain record `(col,row)` | 80×160 macro record lattice | `(col*116 + (row&1)*58, row*32)` |
| Collision cell `(x,y)` | runtime grid 320×160 | no direct draw; same grid domain as Entity/spawn |
| Group spawn `(x,y)` | runtime grid 320×160 | after assignment to Entity, use Entity transform |
| Entity `(x,y)` | runtime grid 320×160 | `(y*58 + (2-(x&1))*29, x*16+16)` |
| Prop raw `(x=+20,y=+1E)` | runtime half-step grid | `(y*58 - (x&1)*29, x*16)` |
| GRAPH placement `(+0C,+0E)` | local pixel anchor | identity |

Then, for draw:

```text
screen = local_world_pixel + camera_offset + sprite/frame offset
```

---

# 12. Original render pipeline and depth behavior

## 12.1 Background terrain

The original draws visible base-terrain records using the 116×32 macro lattice and both byte sublayers.

## 12.2 GRAPH placements

GRAPH pixel-anchor placements are drawn with camera offsets without grid conversion.

## 12.3 Props and actors are not conceptually one permanently baked background

Original code builds runtime draw/depth structures for object props and for Entities. Props carry animation/frame-dependent extents and actors carry projected render coordinates and sprite extents. This is evidence that depth/occlusion is dynamic.

Therefore a remake architecture that permanently bakes every tree/pillar/prop underneath every actor is not fully faithful. It may be temporarily acceptable for bring-up, but it will fail cases where an actor must pass behind a tall prop.

### Required long-term architecture

Prefer:

```text
static terrain/background
GRAPH background placements where appropriate
then dynamic depth queue containing:
    props that participate in occlusion
    actors/entities
    other runtime drawables
```

The exact original depth-key formula is not yet named in this document; preserve sprite bounds and projected Y information so it can be implemented without changing coordinate domains later.

## 12.4 Depth bucket — partial recovery (ORIGINAL-CODE VERIFIED)

`FUN_0042532c` (the world renderer) inserts actors into a **y-bucketed depth
queue**. The bucket index is the actor's screen bottom:

```c
bucket = (Entity+0xEC - camera_y) + (Entity+0xF4 >> 16);
```

i.e. projected screen Y plus the sprite's half-height, where `+0xEC` is the
projected Y from §4/§16.1.5 and `+0xF4` (high word) is the sprite extent. The
queue head array is `0x825468` with a 16-byte slot per bucket and an overflow
chain at `0x81D768`; buckets are then drawn in ascending order up to
`_DAT_0082d178` (the max occupied bucket). This names the previously-unknown
depth key enough to reproduce the ordering; the *special* occlusion flags remain
open (§25 #5).

---

# 13. Portal / transition coordinates

## 13.1 Transition record layout (`Game.N +0x29600`)

**Status: ORIGINAL-CODE VERIFIED for location and trigger rectangle; BINARY-LAYOUT VERIFIED overall**

```text
count  = 250
stride = 0x11

+00 u8 unknown
+01 u8 unknown
+02 u8 meta/type/direction
+03 u8 source_location
+04 u8 destination_location
+05 u16 arrival/target coordinate A   (role strongly indicated, exact naming context-dependent)
+07 u16 arrival/target coordinate B
+09 u16 trigger_x1
+0B u16 trigger_y1
+0D u16 trigger_x2
+0F u16 trigger_y2
```

Original code compares Entity `+08/+0A` directly against the trigger rectangle fields `+09/+0B/+0D/+0F` after checking location.

Therefore transition trigger rectangles are in the **same runtime Entity coordinate domain**.

### 13.1.1 Runtime trigger marking (ORIGINAL-CODE VERIFIED)

At map load (`FUN_0043adf0`) every cell of every transition whose `+03` source
location equals the current location is marked in the runtime collision table:

```c
for (x = trig_x1(+09); x <= trig_x2(+0D); x++)
  for (y = trig_y1(+0B); y <= trig_y2(+0F); y++)
    collision_u16[x*160 + y] |= 0x1000;
```

The scan is bounded by `+00 == 0` (count computed by `FUN_0043a870`). This is the
mechanism behind the `0x1000` marker named in §8.1: gameplay trigger detection
keys off the marked cell, so it never needs the visual `244/245` gate prop.

`+05/+07` (arrival coordinate A/B) read as coordinates inside the `320×160`
runtime grid for real `Game.1` records (e.g. `(24,6)`, `(142,134)`), strongly
supporting the "arrival coordinate" role. The routine that *consumes* `+05/+07`
on arrival was not located: the only static reference to the transition table
base `0x7891b4` is inside the loader itself (`0x43be2e`). Treat the consumer as a
remaining unknown (§25).

## 13.2 Portal visual props are a separate concern

Visual gate/teleport OBJECT props and logical transition records do not need to be bound by nearest-pixel heuristics for core gameplay. The original trigger logic can operate directly from transition records.

Current remake pairing of teleport-looking prop IDs to transition edges is a UI/navigation heuristic unless and until the exact original binding path is reconstructed.

### Canonical rule

For gameplay transition detection:

```text
use Transition trigger rectangle in Entity/runtime coordinates
```

Do not require a visual gate prop to be found first.

For rendering a gate prop, use the prop projection from section 10.

---

# 14. Game.N exact layout

**Status: BINARY-LAYOUT VERIFIED**

For `Game.0`, `Game.1`, `Game.2`:

```text
file size = 0xAD4BE = 709822 bytes
```

```text
offset    count   stride   table
0x00000   8192    0x10     ItemInstance
0x20000    200    0x20     Group
0x21900    500    0x40     WorldObject/runtime-object table
0x29600    250    0x11     Transition
0x2A69A   2000    0x100    Entity
0xA769A     20    0x4B5    Settlement
EOF                         0xAD4BE
```

All boundaries close exactly.

---

# 15. Group[200], stride 0x20

```text
+00 u16 first_entity
+02..+07 unknown

+08 u16 rectA_x1
+0A u16 rectB_x1
+0C u16 rectA_x2
+0E u16 rectB_x2
+10 u16 rectA_y1
+12 u16 rectB_y1
+14 u16 rectA_y2
+16 u16 rectB_y2

+18 u8 current/population-related count
+19 u8 capacity/max-related count
+1A u8 unknown
+1B u8 location_id
+1C u8 entity_count used by contiguous traversal
+1D u8 runtime flags/state
+1E u8 class/activation flags
+1F u8 runtime state/index
```

## 15.1 Group → Entity traversal

**ORIGINAL-CODE VERIFIED**

```rust
first = group.first_entity;
count = group.entity_count;
for id in first .. first + count {
    entity = Entity[id];
}
```

Entity stride is `0x100`.

A Group is not a drawable actor.

## 15.2 Active population groups

Original location setup iterates group indices beginning at **1**, checks `Group+0x1E != 0`, checks location, and uses `+0x1C`/population state.

Group 0 is reserved/special in this population path.

## 15.3 Spawn rectangle A

Initial runtime placement uses:

```text
x1 = +08
x2 = +0C
y1 = +10
y2 = +14
```

The Group record is interleaved. Do not decode `+08,+0A,+0C,+0E` as one rectangle.

Rectangle B is:

```text
x1 = +0A
x2 = +0E
y1 = +12
y2 = +16
```

Its exact long-form semantic name remains **UNKNOWN** until separately proven.

---

# 16. Original runtime placement algorithm

**Status: ORIGINAL-CODE VERIFIED**

Routine around `0x411DE4`.

For rectangle A:

```text
half_x   = trunc_toward_zero((x2-x1)/2)
half_y   = trunc_toward_zero((y2-y1)/2)
center_x = trunc_toward_zero((x1+x2+1)/2)
center_y = trunc_toward_zero((y1+y2+1)/2)
```

Up to 100 attempts:

```rust
if half_x < 2 {
    dx = 0;
} else {
    dx = rand() % half_x;
    if rand()%2 == 0 { dx = -dx; }
}

if half_y < 2 {
    dy = 0;
} else {
    dy = rand() % half_y;
    if rand()%2 == 0 { dy = -dy; }
}

x = center_x + dx;
y = center_y + dy;
```

The candidate is written directly to `Entity.x/y` and accepted only if:

```rust
0 <= x < 320
0 <= y < 160
(collision[x*160+y] & 0x0FFF) == 0
```

If all 100 attempts fail, the routine returns failure. **No center fallback.**

The original caller may shorten the group's active count when placement fails; do not silently pretend the actor was placed.

Exact failure mechanism (ORIGINAL-CODE VERIFIED, `FUN_0043adf0`): on the first
failed placement (`FUN_00411de4` returns 0) the loader writes the **ordinal of
the failing entity** into `Group+0x1C` and stops placing that group.

Bounds note: `FUN_00411de4` itself tests **only** the collision cell
(`(collision[x*160+y] & 0x0FFF) == 0`). There is no explicit
`0 <= x < 320` / `0 <= y < 160` test inside the routine; in-range candidates are
guaranteed by the rectangle it is handed. The bounds check listed above is
therefore defensive, not a literal original instruction.

---

# 16.1 Original pathfinding and movement

**Status: ORIGINAL-CODE VERIFIED** (`konung.res`; addresses are VAs in the
x86 PE, image base `0x00400000`).

Movement is a **direction-code queue**, not a list of cells and not a
screen-vector `atan2` facing.

## 16.1.1 Issuing a walk command — `FUN_004142D4(entity, tx, ty, mode)`

```c
entity + 0x0C/u16 = tx;          // target_x
entity + 0x0E/u16 = ty;          // target_y
entity + 0x02/u8  = mode;        // flags_a (0x48 for wander)
entity + 0x14/u8  = 0xFF;        // clear queue terminator
if (FUN_0043DA1F(entity) == 0) return 0;   // no route -> command fails
entity[2] &= ~0x40;
FUN_0041435C(entity, (entity[3] & 4) ? 1 : 7);  // walk, battle/peaceful
return 1;
```

The target fields are written **before** pathfinding, so a failed command still
records the destination but does not start moving. A blocked or unreachable
point therefore never moves the actor ("moving starts from a click on an
acceptable point").

## 16.1.2 Route builder — `FUN_0043DA1F(entity)`

A weighted grid search over the 320×160 collision table
(`DAT_004EE6B0`, `index = x*160 + y`, low-12 free). It writes the direction used
to *enter* each visited cell and finally stores the forward direction list in
`entity + 0x13` (32 bytes, terminator `-1` at `+0x32`), i.e. **at most 31
steps**.

If the destination cell is blocked (low-12 non-zero), the routine aborts and
returns 0 (except the entity's own interaction target).

## 16.1.3 The 8 movement directions are parity-dependent — `FUN_0043D922`

Neighbour deltas come from two 8-entry `{i16 dx, i16 dy}` tables selected by the
parity of the current cell's `x` (the staggered isometric lattice), read
verbatim:

```text
x even (0x480110): 0:(0,-1) 1:(-1,0)  2:(-2,0) 3:(-1,1) 4:(0,1) 5:(1,1) 6:(2,0) 7:(1,0)
x odd  (0x48010C): 0:(0,-1) 1:(-1,-1) 2:(-2,0) 3:(-1,0) 4:(0,1) 5:(1,0) 6:(2,0) 7:(1,-1)
```

`FUN_0043D922(out_x, out_y, x, y, dir)` writes `(x+dx, y+dy)` and validates
`0..320 × 0..160`. Edge weights in the search are 10 for dirs `1/3/5/7` and 14
for dirs `0/2/4/6`. Ported as `coords.rs::DIR_EVEN`/`DIR_ODD`,
`coords.rs::step_by_direction`, `coords.rs::neighbors`,
`coords.rs::direction_cost` and `actors.rs::find_path`.

## 16.1.4 Advancing the queue — `FUN_004123BC`, `FUN_0041435C`, `FUN_00413DE4`

Each logic frame advances the animation (`FUN_00413DE4`). A walking entity steps
**one cell per completed walk cycle** and only when
`Entity+0x01 (facing) == Entity+0x13 (queue head)` (`FUN_004123BC`); after the
step the queue is shifted left (`FUN_0043EA79(+0x13, +0x14, 0x1F)`). Because the
facing must equal the pending direction, a straight walk keeps a **constant
facing** — the remake's old BFS-cell + `atan2` facing produced the zigzag.

**Commit timing (verified):** the actual cell step is performed by the *tail* of
`FUN_0041435C`, which runs whenever the action is walk (`5`/`1`/`7` reaching that
path) and the queue head `+0x13 != 0xFF`: it calls `FUN_0043D922` from
`+0x08/+0x0A` and writes the resulting neighbor back into `+0x08/+0x0A`.
`FUN_0041435C` is invoked (a) immediately after a route is built and (b) after
each `FUN_004123BC` pop, so **each step is committed at the START of its walk
segment**. Because `FUN_0041435C` first stores the pre-step projection into
`+0xE8/+0xF0` (see §16.1.5) and draws with the `+0xE0` frame dx/dy anchor, the
stored cell leads the sprite by exactly one step during the cycle. A mid-walk
move command therefore re-plans from the cell being walked INTO, not the cell
being left (`app.rs` `player_render_x/y`/`player_active`;
`actors.rs::ActorMotion.render_cell`/`walking`).

Facing toward a target without moving uses `FUN_0043897C` (0 = −y, 4 = +y,
2 = −x, 6 = +x, 1/3/5/7 the diagonals), ported as
`coords.rs::direction_from_cells`.

## 16.1.5 Actor render projection — `FUN_00438A78`

`FUN_00438A78(out_x, out_y, x, y)`:

```text
*out_x = y*58 + (2-(x&1))*29
*out_y = x*16 + 16
```

`FUN_0041435C` calls it with `Entity+0x08/+0x0A`, stores the result into the
runtime render fields `Entity+0xE8/+0xF0`, and derives screen coordinates
`Entity+0xE4 = camera_x + +0xE8 + frame_dx`,
`Entity+0xEC = camera_y + +0xF0 + frame_dy`. It **is** therefore the actor render
projection (see §4); unlike the separate prop (§10.3) and terrain (§7.2)
lattices. Ported as `coords.rs::entity_to_local_px`.

---

# 17. Entity[2000], stride 0x100

Canonical known persisted fields:

```text
+00 u8   kind
+01 u8   facing/direction (0..7)
+02 u8   flags_a
+03 u8   flags_b
+04 u8   archetype/class flags
+05 u8   group_id
+06 u16  target entity/relationship id (context-dependent)

+08 u16  x                    runtime/collision grid
+0A u16  y                    runtime/collision grid
+0C u16  target_x
+0E u16  target_y

+10 u8   animation_frame
+11 u8   movement/AI parameter ?
+12 u8   action parameter; humanoids use known values as combat/equipment slot
+13..+32 path/action queue, 32 bytes, 0xFF unused/terminator

+33 u8   level
+34 i32  money
+38 u32  available XP
+3C u32  invested/spent XP

+40 u16  melee mastery ×100
+42 u16  trade skill
+44 u16  archery mastery ×100
+46 u16  building skill
+48 u16  healing skill
+4A u16  identification/rare-item skill
+4C u16  smithing skill
+4E u16  base armor
+50 u16  periodic damage / DOT accumulator
+52 u16  venom/poison value carried into the corpse loot stinger

+54 u16  dexterity_base
+56 u16  dexterity_current
+58 u16  charisma_base
+5A u16  charisma_current
+5C u16  hp_internal (display ~= (hp+15)/16)
+5E u16  strength_base
+60 u16  strength_current
+62 u16  endurance_base
+64 u16  endurance_current

+66..+71 equipment[6]  as six u16 ItemInstance ids
+72..+B1 inventory[32] as 32 u16 ItemInstance ids
+B2..+BB unknown

+BC u16 saved dexterity/current backup
+BE u16 saved charisma/current backup
+C0 u16 saved strength/current backup
+C2 u16 saved endurance/current backup
+C4..+CD temporary-effect state, partly unresolved
+CE u16 temporary-effect timer

+D0 u8 dialogue_id
+D1 u8 status flags
+D2 u8 unknown
+D3 u8 unknown
+D4 u8 name_id
+D5 u8 nickname_id
+D6 u8 visual_index
+D7 u8 unknown
+D8 u32 palette_offset / palette-base offset
+DC..FF runtime animation/render state; not all fields persisted/semantically named
```

## 17.1 Runtime render fields in memory

Original setup computes projected pixel coordinates from Entity x/y and stores them in runtime fields used by the renderer (notably around `+E8/+F0`). These are derived state and should not replace persisted grid x/y in the remake model.

---

# 18. Entity visual selection

**Status: ORIGINAL-CODE VERIFIED**

Renderer family selection is based on:

```rust
entity.archetype & 0x40
```

not `kind == 6`.

```text
bit clear -> humanoid/base graphics family, block size 0x4D28
bit set   -> object/monster/special family, block size 0x3228
```

`Entity+D6` is a family-local visual index.

`Entity+D8` is palette/palette-base data, not a sprite-template id.

Canonical names:

```rust
visual_index: u8;      // D6
palette_offset: u32;   // D8
palette_id = palette_offset / 512; // current resource-layout convention
```

Do not fall back to "any matching human/monster" when resolution fails. Return an explicit error.

---

# 19. ItemInstance[8192], stride 0x10

```text
+00 u8 kind/category; 0..5 map to equipment slot index
+01 i8 condition/quality byte; -1 none (not the damage modifier)
+02 i8 damage-modifier index (weapon-vs-creature table `0x462798`); -1 none
+03 u8 prototype_id
+04 u32 union value
+08 u32 union max/secondary value
+0C u16 poison
+0E u16 flags
```

**Damage-modifier index (ORIGINAL-CODE VERIFIED).** `FUN_0041aea8`
(`0x41aea8`) reads `byte [instance + 2]` as the index into the 18-record
weapon-modifier table at `0x462798` (see §20). The `+0x01` byte is retained as
`condition`; it is not that index.

Durable equipment commonly interprets:

```text
+04 f32 current durability
+08 f32 max durability
```

Stackables/ammunition use:

```text
+04 u32 quantity
```

Known flag:

```text
+0E bit0 -> unidentified/unknown presentation path
```

**Instance-table source (ORIGINAL-CODE VERIFIED).** The table that resolves a
loaded game's inventory/equipment ids is the *loaded save file* (`Game.0/1/2`)
at offset `0` (8192 × 16, §14). The original loader `FUN_0043a870` (string
`GAME.` @ `0x450747`) reads `0x20000` bytes of it into memory. The separate
`Game.res` file is **not** this table for a loaded game (it supplies the variant
table read by the renderer); resolving save ids against `Game.res` yields wrong
items. Class = `+00`, subtype = `+03` (the selector `FUN_0042ba40` reads
`*param >> 0x18` as subtype and the low byte as class).

---

# 20. ItemPrototype, stride 0x40

Prototype table resides in original executable/resource data.

```text
+00..+2B name[44], CP866, NUL terminated
+2C u16 combat_value
+2E u16 durability
+30 u16 requirement_stat
+32 u16 requirement_value
+34 u16 range/reach
+36 u16 price
+38 u16 weight_grams
+3A u16 inventory_icon_id
+3C u16 world_sprite_id
+3E i16 equipped_visual_id / overlay set; -1 none
```

**Table bases (ORIGINAL-CODE VERIFIED).** The class selects a table
(`FUN_0042ba40` / `FUN_0042f5f8`). The icon/weight fields are addressed from the
selector base (`icon = u16@+0x02`, **`weight = i16@+0x00` — SIGNED**); the CP866
name lives in a **parallel table with a different base**, read at `+0x00`:

```text
class 0..=5 or 12 -> icon/weight 0x459B88   name 0x459B50   stride  64
class 6..=8       -> icon/weight 0x45812A   name 0x4580B0   stride 128
class 9           -> icon/weight 0x4594CA   name 0x459450   stride 128
class 10..=11     -> icon/weight 0x460996   name 0x46091C   stride 128
```

**Signed weight (ORIGINAL-CODE VERIFIED).** Both weight readers load the dword at
`base + subtype*stride` and take `>> 0x10` (arithmetic shift ⇒ the high word is
sign-extended):
`FUN_0041B0C4` (single instance) and `FUN_0041B7E4` (total carried weight over
the 6 equipment + 5 additional + 32 inventory slots + `+0xCC`), both returning
`int` with no clamp. A negative template weight therefore *reduces* the carried
total. The only such item in the shipped data is class `11` subtype `11`
«Сын Луны»: `0xD8F0` = −10000 (`−10.0 kg`). Class `12` stacks multiply the
signed weight by the stack quantity (`FUN_0041B7E4` class-12 branch).

```text
CHECK 1  domain: template record field, internal weight units (display /1000)
CHECK 2  consumers: FUN_0041B0C4, FUN_0041B7E4 (sign-extended high word)
CHECK 3  conversion: arithmetic >> 0x10 sign-extends; no abs, no clamp
CHECK 4  remake: now i32 end-to-end (item_weight/carried_weight/capacity)
CHECK 5  no scale/clamp/fallback added beyond the original `int` sum
```

For the stride-`0x40` table the field offsets above (`+0x00` name … `+0x3E`
equipped-visual) hold exactly. The stride-`0x80` tables use the same `name@+00`
and `icon@+0x02` (relative to their selector base), but the remaining field
offsets differ and are **not** yet fully named. Do **not** read the name at
`icon_base + 8` / `+ 6`; that lands one record late (`+0x40` / `+0x80`).

**Class 10 caveat (ORIGINAL-CODE VERIFIED).** The *selector* `FUN_0042ba40`
groups classes `10..=11` on base `0x460996`, but the *name/detail* routine
`FUN_0042f5f8` has **no `case 10`**: it falls through to its empty `default`, so
only class `11` gets a name there. Treat class `10` as icon/weight-only until a
name path is found.

**Class-11 item use (`FUN_00414DE4`).** Class `0x0B` instances are dispatched by
the subtype byte `+0x03` when used from the bag (cursor base `9`); the item is
consumed by writing `+0x00 = 0xFF`. The full ~50-subtype catalogue, the C1–E
implementation status and the per-case effects are in `CLASS11_ITEMS.md`. Two
outcomes matter to other subsystems:

- **Subtype 11 «Сын Луны»** has no case. It is a passive artefact whose signed
  template weight is −10000 (see above), so it is never consumed.
- **Subtype 24 «Лопата»** (case `0x18`) only sets interface mode `0`
  (`_DAT_0081D610`), base cursor `7` (`_DAT_0081D660`) and sound 10; it is not
  consumed. Digging is the *carried* shovel gate in §28.7, not a use effect.
- **Subtype 33 «Кукла»** (case `0x21`) writes `Entity+0xD2 = 0x13` for every
  living actor of every other group in the current location, then consumes.
- **Subtype 38 «Святые мощи»** writes protagonist `+0xD3 = 0x46`.

**Weapon modifier table (BINARY-LAYOUT VERIFIED).** `0x462798`, 18 records of
`0x40` bytes: `+0x00 u16 damage_value`, `+0x02 u16 target_archetype`,
`+0x14` CP866 name suffix. `FUN_0041aea8` uses the record value in place of the
prototype rating when `instance+0x02` is in `0..=17` and the target entity's
`Entity+0x04` equals the record's target archetype. Verified records include:

```text
1  5000 / 0x55  ", на муравьев"
3  3000 / 0x46  ", на пауков"
15 9000 / 0x44  ", на ядовитых пауков"
16 2000 / 0x4F  ", на ядовитых червяков"
```

`equipped_visual_id` (`+0x3E`) is the actor-overlay selector: it indexes the 24
GRAPH human-overlay blocks (`0..=23`, `-1` = none). A Game.N entity's worn items
are the six `u16` ids at `Entity+0x66` (slot index `0..=5`; class/subtype come
from the item instance table). `FUN_00427e9c` composites them onto the humanoid
base frame:

```text
peaceful (Entity+0x03 & 4 == 0):
    order = DGROUP 0x4631FC[facing][6]          (slot order)
    no selector variant

battle (Entity+0x03 & 4 != 0):
    bucket = (action 1 -> 3, action 5 -> 2, action 4 -> 1, else 0)
    if Entity+0x12 == 1 and bucket != 2:
        order = DGROUP 0x462EFC[bucket][facing][6]
        variant slot = 1
    else:
        order = DGROUP 0x462BFC[bucket][facing][6]
        variant slots = { Entity+0x12 (unless 1), 5 }
```

For each slot in the order, the item's `equipped_visual_id` is read from the
stride-`0x40` prototype table by subtype; if the slot is a variant slot the
selector is advanced by one (the alternate "in hand" GRAPH block) before the
overlay is drawn. This is the port's `res.rs::overlay_plan` /
`compose_human` (`HUMAN_DRAW_ORDER_PEACE`, `HUMAN_DRAW_ORDER_BATTLE_A/B`).

The character-sheet paper doll is `FUN_0042ea6c`: the same base+overlay frame is
rendered with the base sprite's top-left at `(0x3C,0x14)` inside a 160x160
region and blitted at `DGROUP 0x46035C`/`0x460360` (640 -> `(470,127)`,
800 -> `(570,160)`).

Requirement-stat ids:

```text
1 Хар -> charisma
2 Лов -> dexterity
3 Хар -> charisma (duplicate original id)
4 Здр -> hp/16
5 Сил -> strength
6 Вын -> endurance
7 Брн -> calculated armor
8 Удр -> calculated attack
9 Вер -> derived calculated value; full long-form label still unresolved
```

**Label table verified (BINARY-LAYOUT VERIFIED).** The nine requirement labels
are contiguous 5-byte CP866 strings at VA `0x415E05` (inside `FUN_00415c40`),
indexed by `requirement_stat - 1`:

```text
0x415E05 Хар  0x415E0A Лов  0x415E0F Хар  0x415E14 Здр  0x415E19 Сил
0x415E1E Вын  0x415E23 Брн  0x415E28 Удр  0x415E2D Вер
```

Ids `1` and `3` both render `Хар` (charisma). `Вер`'s long form is still open
(§25 #2).

---

# 21. Canonical implementation contract by render layer

An implementation agent should not need to ask which projection to use. Use this table.

## 21.1 Base terrain

```rust
for row in visible_rows {
    for col in visible_cols {
        let (lx,ly) = (col*116 + (row&1)*58, row*32);
        draw layer0 if nonzero, resource id layer0-1;
        draw layer1 if nonzero, resource id layer1-1;
    }
}
```

Apply camera after computing local coordinates.

## 21.2 GRAPH placements

```rust
local_x = record.x_px; // +0C
local_y = record.y_px; // +0E
```

No grid projection. Apply camera and sprite offsets.

## 21.3 Props

Parse raw:

```rust
grid_x = u16(+0x20);
grid_y = u16(+0x1E);
```

Project:

```rust
local_x = grid_y*58 - (grid_x&1)*29;
local_y = grid_x*16;
```

Then camera + object/frame offsets.

## 21.4 Entities, including protagonist and mobs

```rust
local_x = y*58 + (2-(x&1))*29;
local_y = x*16 + 16;
```

Then camera + actor/frame offsets.

## 21.5 Runtime-spawned `(0,0)` entities

Generate `(x,y)` from Group rect A, validate with `collision[x*160+y]`, store as runtime actor position, then use the **same Entity projection** as persisted actors.

No provisional accept-all validator remains in canonical mode.

## 21.6 Logical transitions

Detect against Transition trigger rectangle in Entity/runtime grid coordinates. Rendering a visual gate prop is independent.

---

# 22. Known current-remake deviations to fix before declaring C2 complete

The following are known deviations if still present in the working tree:

1. **Half-scale base terrain:** `58×16` / `29` record lattice instead of `116×32` / `58`.
2. **Only low byte of terrain u16 rendered:** high-byte second terrain layer omitted.
3. **Old actor/transition projection:** `29+x*29,y*16` instead of Entity projection.
4. **Prop projection tied to terrain lattice:** wrong. Use section 10.
5. **C2 accept-all placement validation:** wrong. Use collision table directly.
6. **Props permanently baked below all actors:** acceptable only as temporary bring-up; not faithful occlusion.
7. **Portal binding via nearest visual props:** heuristic, not required for logical transition detection.

These are not questions for an implementation agent. They are concrete correction items.

---

# 23. Required regression/unit tests

## 23.1 Entity projection

```text
(0,0) -> (58,16)
(1,0) -> (29,32)
(2,0) -> (58,48)
(5,7) -> (435,96)
```

## 23.2 Prop projection

```text
(x=0,y=0) -> (0,0)
(x=1,y=0) -> (-29,16)
(x=2,y=0) -> (0,32)
(x=5,y=7) -> (377,80)   // 7*58 -29, 5*16
```

## 23.3 Terrain lattice

Ignoring camera:

```text
terrain (0,0) -> (0,0)
terrain (1,0) -> (116,0)
terrain (0,1) -> (58,32)
terrain (1,1) -> (174,32)
```

## 23.4 Collision

```text
index(0,0)     = 0
index(0,159)   = 159
index(1,0)     = 160
index(319,159) = 51199
```

Must explicitly test X values >159, e.g. 268.

## 23.5 Terrain record dual layers

For a synthetic u16 record `0x0201`, the renderer must attempt to draw resource indices:

```text
layer0 byte 1 -> resource 0
layer1 byte 2 -> resource 1
```

not one tile id `1` or `513`.

---

# 24. Milestone order after this spec revision

Do not restart a large mixed task. Use narrow phases.

```text
R1  Correct base-terrain lattice to 116×32/58 and render both terrain bytes.
R2  Correct Entity/protagonist projection to the original transform simultaneously.
R3  Correct prop raw-grid projection; leave GRAPH placements as direct pixels.
R4  Restore C2 collision validation (x*160+y), remove provisional accept-all.
R5  Visual regression: protagonist, C1 persisted actors, C2 runtime actors, props all align.
R6  Improve prop/actor depth interleaving if needed.
R7  Population growth beyond current entity_count.
R8  Stats.
R9  Inventory/equipment UI.
```

Do not implement R2 without R1; that exact partial change caused the previous visual regression.

---

# 25. Explicit remaining unknowns

These are legitimate unknowns. They should **not** trigger questions during unrelated implementation.

1. Exact semantic name of Group rectangle B.
2. Exact long-form meaning of derived stat label `Вер`.
3. Complete semantic naming of Entity AI/effect tail fields.
4. Exact original global RNG sequence/state implementation for spawn reproducibility.
5. Exact original prop/actor depth-sort key and all special occlusion flags.
   (Partially resolved — see §12.4: the depth bucket is the sprite's screen
   bottom `(screen_y + sprite_half_height)`. The special occlusion flags are
   still unknown.)
6. Exact logical association, if any, between visual teleport props and Transition records. Gameplay transition triggers themselves are already known.
7. Some WorldObject (`Game.N +0x21900`) fields and behavior.
   (Resolved for the gameplay fields: layout, activation, sprite, buried
   records and the shovel/mirror gates are documented in §28.7. Still unknown:
   the `+0x1E` prop-linkage payload beyond the settlement variant pass and the
   exact semantics of the `+0x0C` sprite override.)

If a task does not require one of these, preserve it as unknown and proceed.

---

# 26. "Agent should not ask" decision matrix

When implementing map/population rendering:

```text
Question: Which terrain spacing?
Answer: 116×32, odd row +58.

Question: One or two bytes per terrain record?
Answer: two independent 1-based drawable byte layers.

Question: Which Entity projection?
Answer: x_screen_local = y*58 + (2-(x&1))*29; y_screen_local=x*16+16.

Question: Which prop projection?
Answer: raw x=+20, raw y=+1E; local=(y*58-(x&1)*29, x*16).

Question: Which GRAPH placement projection?
Answer: none; +0C/+0E are already pixel anchors.

Question: Group spawn coords same as Entity/collision coords?
Answer: yes, original placement writes them directly to Entity x/y and immediately indexes collision.

Question: Bounds for runtime grid?
Answer: x 0..319, y 0..159.

Question: Placement validation?
Answer: low 12 collision bits must be zero; max 100 attempts; no center fallback.

Question: Human vs object/monster renderer family?
Answer: Entity.archetype & 0x40.

Question: D6 / D8?
Answer: D6 visual index; D8 palette offset/base, not sprite template.

Question: Portal gameplay detection tied to visual prop?
Answer: no; logical transition trigger rectangle is sufficient.

Question: Allocate a giant full-map surface?
Answer: not required. Original is viewport-based; surface size is not world bounds.
```

---

# 27. Final invariant

Every coordinate-bearing record must have its domain named in the type or variable.

Bad:

```rust
let x = record.x;
let y = record.y;
```

Preferred:

```rust
let entity_cell_x = entity.x;
let prop_grid_x = prop.raw_x20;
let graph_pixel_x = graph.x_px;
let terrain_col = col;
```

If code requires an unexplained conversion between two of these, stop that conversion, not the whole task. Search the original binary or leave the conversion isolated as UNKNOWN. Never make the rest of the world conform to a guessed scale.

---

# 28. Binary-only systems present in `konung.res` but not in the remake

Revision 4 recovered the following original runtime mechanisms. They are
ORIGINAL-CODE VERIFIED in `konung.res` (unless marked otherwise) and are **not**
implemented in `src` at the time of writing. Treat each as a documented
feature gap, not a question.

## 28.1 Population growth (`FUN_00420a8c`)

Gated on: `Group+0x1E & 0x40` set (active), the group's location (`+0x1B`)
matching the current location, and `Group+0x1E - 0x41 < 0x10` (i.e. `+0x1E` in
`0x41..=0x50`). Growth then runs while `Group+0x1C < Group+0x18` and the periodic
population pass keys off `_DAT_0081d640 & 0xF`.

**Implemented (`src/actors/growth.rs`).** `FUN_0043ADF0(mode 0)` is the only
caller: on map entry `grow_map` visits groups `1..200` and fills a group while
signed `Group+0x02 < 0` and `(char)+0x1C < (char)+0x18`. Each created member
adds the class interval high word (`0x45E216 + class*4`) to `+0x02`; the pass
then clamps a still-negative accumulator to `0`. The original initialises a
`local_2c` flag to `1`, so the **first** eligible group of the location is
skipped and every later one grows (reproduced verbatim). `FUN_0041BD3C` runs in
the 16-frame periodic pass: it raises `+0x18` by one when the signed countdown
`+0x06` underflows (re-armed from the low word of `0x45E31E + (class-0x41)*4`),
and decrements `+0x02` while `+0x1C < +0x18`. `FUN_00420A8C` averages
`+0x54/+0x58/+0x33/+0x34/+0x4E/+0x52/+0x5E/+0x11/+0x62`, sets HP `+0x5C =
0x640`, clears equipment/inventory/additional, copies `+0xD6/+0xD8` from a
random member, sets `+0x05` to the group index, `+0x03 = 4`, `+0xD0 = 0xFF` and
class `L` additionally gets `+0x12 = rand % 5 + 1`; the constructor places once
and the caller places again. Growth state (`+0x02`/`+0x06`) is persisted in
`GroupExtra` (save v3). Save/load mode `1` never grows.

## 28.2 Actor occupancy and the interaction exception

Actors OR their entity index into the collision `u16` and clear it on leaving
(§8.1). `FUN_0043da1f` accepts a blocked destination only when
`(cell & 0x0FFF) == Entity+0x06`. **Implemented**: `find_path_with_exception`
accepts a blocked goal only when the cell's low 12 bits are non-zero, not
`0xFFF`, and equal the ordered target id (`Entity+0x06`); the order low bits are
non-zero. Ordinary `find_path` remains the no-exception form.

## 28.3 Entity-type activity table `0x81B954`

A 512-entry (`0x200`) table of `i32` states (`-1` disabled / `0` free /
positive active), reset per map load and incremented when actors of a type are
placed (`FUN_0043adf0`). It gates which entity types are active on a map. The
remake has no equivalent.

## 28.4 Group activation / traversal gate

Beyond `first_entity + entity_count` (§15.1), the loader and population pass
gate on `Group+0x1E != 0`, `Group+0x1B == location`, and the count comparison
`Group+0x1C < Group+0x18`. Population growth is not implemented (§28.1).

## 28.5 NPC / monster action machine (`FUN_004123bc`)

A per-entity action state machine driven by `Entity+0x00` stance tokens
(`M`/`O`/`L`/`I` …) with transitions through `FUN_00411fd0`, `FUN_00414078`,
`FUN_004116cc`, `FUN_00412e00`, and death/attack setup in `FUN_0041435c`. The
remake implements only idle/walk.

Two recovered details used by the remake:

- **Idle is held, not looped for `flags_a & 0x40` entities.** The animation
  switch is entered directly only when `(Entity+0x02 & 0x40) == 0` (or action
  `2`/`3`/`9`); a `0x40` entity is routed through `FUN_004116cc` (0x4116CC:
  `[0x81d5f0] < 3 && entity > [0x81d568]`) into `FUN_00412e00`, which returns `0`
  while idle, so its idle cases `0`/`6` are not reached and the frame is held.
  Every entity in the shipped active groups is `flags_a = 0x40`.
- **Object/monster per-frame anchor table.** `FUN_00414078` (object branch
  `0x41410a`-`0x414143`) computes gx/gy at
  `block + 0x1728 + row*0x480 + direction*0x90 + fi*8` (gx at `+4`, gy at `+6`),
  i.e. indexed by `(row, direction, frame-index)`, **not** by the animation frame
  id. The sprite itself is `block + 0x728 + frame_id*8`. The human GRAPH anchor
  is the analogous `block + 0x2F28 + action*0x300 + direction*0x60 + fi*8`.
- **AI selection is by group class, not entity flags (`FUN_00421b50`,
  0x421B50).** For each active group whose `Group+0x1B == current location`:
  `class(+0x1E) & 0x40` → `FUN_004123bc` (AI = `FUN_00412e00`); `class & 0x40 ==
  0 && class & 0x80` → `FUN_00415fc8` (AI = `FUN_004168a4`, waypoint/wander);
  both bits clear → inline AI `FUN_0041ecc8`. `FUN_00412e00` with
  `Group+0x1D == 0` only normalizes the action to idle `0`/`6` and returns
  **without moving**, so **object/enemy groups (e.g. the giant ants, class
  `0x41`) idle in place**; only citizen groups (class `0x80`/`0x81`/`0x82`) run
  the waypoint/wander AI. `FUN_0043adf0(mode)` (mode 0 = new game / map entry;
  callers `FUN_00436214`) clears `Group+0x1D` for `class & 0x40` groups; mode 1
  (load save, `FUN_004246c0` from `KONUNG.SA?`) keeps it.
- **Group-relation pass (`FUN_00412c30`, called from `FUN_0041d048` every 16
  logic frames).** For each current-location `class & 0x40` group:
  any party member inside rectangle A -> `Group+0x1A = party group` and
  `Group+0x1D = 1`; otherwise any own member outside rectangle A ->
  `Group+0x1D = 0x20` (return/recall, with opportunistic `FUN_0041fb64`
  chase); otherwise `0`. `FUN_004245b4` only sets activity `1` and clears
  `Entity+0x14` for `flags_a & 1 == 0`/`archetype != 0x49`; it does not assign
  a target. `FUN_00412e00` activity `0x20` is therefore a return-to-rectangle
  branch, not generic patrol. Citizen groups keep their authored `0x1D = 0x80`
  (mode 0 clears only `class & 0x40`) and do not enter the `FUN_004168a4`
  waypoint/wander branch until woken to `1`.
- **M/O walk delay (`FUN_004123bc`).** For archetypes `0x4D`/`0x4F`, after a
  walk cycle `Entity+0x12` is a countdown: if zero, seed
  `1 + (rand%2) * rand%(Entity+0xD7)`, else decrement; the next route/action is
  considered only when it reaches zero. Humanoid `Entity+0x12` remains the
  active weapon slot.
- **Wander radii (`FUN_0042781c`, 0x42781C).** `_DAT_0081D6E0`/`_DAT_0081D6A4`
  are set statically: 800x600 → `20`/`9`; 640x480 → `17`/`7`. `FUN_00413a84`
  samples `rng() % (2*radius)`.

## 28.6 Settlement → prop linkage (`FUN_0043adf0` / `FUN_004172ac`)

For the settlement whose location matches, sub-records (`stride 8`: `+0` target
key, `+1` state, `+2` prop index, `+6` timer) patch the prop runtime table:

```text
object_id = 0x458A02[key].base(u16@+2) + state + 30
```

The `+30` is the placement/object-table offset (verified against `Konung.N` map
props). `FUN_004172ac` also copies the object record's `+0x14` palette and
reinitialises its animation frame and FLAMES phases. State `0..2` is
construction, `3` is complete, `4..6` is damage/ruin and `6 -> 0` rebuilds.
Implemented in `app/world_items.rs::apply_settlement_prop(s)`; the target timer
pass is `app/settlement_targets.rs`.

Waypoint `+1` (the authored role dialogue id) is likewise implemented:
`0x14C`/`0x159`–`0x15C` copy it to `Entity+0xD0` when a specialist is stationed.
See `SETTLEMENT_SYSTEM.md`.

## 28.7 WorldObject runtime table (`0x77DEB4`, 500 × 0x40)

Record layout (`Game.N +0x21900`, `FUN_0043A870` loads it verbatim):

```text
+0x00 i32  activation timestamp 0        +0x04 i32 activation timestamp 1
+0x08 u16  runtime-grid x                +0x0A u16 runtime-grid y
+0x0C u16  ground sprite id              +0x0E u16 source index (0 => WIN.FLM)
+0x10 i16  money  (negative = BURIED)    +0x12 u16 location id (0 = removed)
+0x14..+0x3F u16 x22 item-instance ids
```

Active world objects carry `+0x08`/`+0x0A` runtime-grid coordinates that are run
through the **Entity** projection (`FUN_00438a78`) and drawn in their own depth
pass. `FUN_0043ADF0` filters by location (`+0x12`) and by the activation pair
(`+0x00 >= 0`, or `+0x00 != +0x04`, or `now + +0x00 >= 0x1518`); the money sign
does **not** affect activation.

**Buried records / digging (ORIGINAL-CODE VERIFIED).** A record with
`money (+0x10) < 0` is a buried treasure:

- `FUN_0042532C` marker pass draws only when `-1 < (i16)+0x10`; buried records
  are never drawn, with or without the torch (`_DAT_0081D628`).
- `FUN_00428740` (`0x428A2B`) and the hover branch `0x423CD2` also require
  `money >= 0`; the hover message is `0x45C31C` «Что-то лежит на земле».
- Arrival pickup in `FUN_00421B50` is gated:
  `cell & 0x2000 && (money >= 0 || FUN_00433A8C(0x19) != 0)`. `FUN_00433A8C(n)`
  scans the leader's 32 inventory slots (`Entity+0x72..+0xB1`) for a class
  `0x0B` instance with `subtype == n-1`; `0x19` ⇒ subtype `0x18` = 24 =
  «Лопата». Equipment/additional slots do not count.
- `FUN_004110EC` pays `FUN_0043EAA2(+0x10)` = `abs(money)`, then clears the
  marker's money and transfers the items.
- The copper mirror (class-11 subtype 48, `FUN_00414DE4` case `0x30`) negates
  `+0x10` for every record of the current location, making the spots visible and
  pickable without the shovel; it is consumed.
- Emptied records: `FUN_0041093C` respawns only a sole class-9 subtype-1/2 item;
  everything else gets `+0x12 = 0` (permanent).

There are **92** buried records in each shipped `Game.0/1/2` (identical
coordinates, profile-specific item ids). Full table: `DIG_SYSTEM.md`. The
«Лезвие древнего меча» (class 11 subtype 23) is at location `15`, cell
`(50,49)`.

```text
CHECK 1  domain: WorldObject table cell = runtime/Entity 320x160 grid
CHECK 2  consumers: FUN_0043ADF0 (pool), FUN_0042532C (draw), FUN_00428740 /
         0x423CD2 (cursor/hover), FUN_00421B50 + FUN_004110EC (pickup)
CHECK 3  conversions: +0x10 i16 sign; money paid through FUN_0043EAA2 (abs)
CHECK 4  remake: buried records kept in the active pool, hidden, shovel-gated
CHECK 5  no clamp/fallback added: money sign and abs come from the original
```


## 28.8 Terrain-extent scan (`FUN_0043adf0`)

At load the runtime scans non-zero terrain to compute `_DAT_0081d758`
(`max_row * 32`, vertical extent) and `_DAT_0081d73c` (`(max_col-1) * 116`,
horizontal extent). These bound scrolling/culling. The remake derives extents
from its own lattice constants, not from a data scan.

## 28.9 Ground-shadow span pass (`FUN_0043C040` / `FUN_0043D247`)

**Status: ORIGINAL-CODE VERIFIED + IMPLEMENTED.**

The original has **no dirty-rectangle system**. It keeps 600 per-scanline heads
(`DAT_004C6B50`), each a sorted linked list of `{x0, x1, next}` runs in a fixed
run arena (`0x5076FC`, budget `_DAT_005076D8`).

```text
FUN_0043C040(mask, screen_x, screen_y)
    w = u16@mask+0, h = u16@mask+2
    h x u16 row sizes follow, then the row payloads
    row: 0x00 = end, byte & 0x80 = skip (byte & 0x7F), else a run of `byte` px
    clip x to [0x5076E4..0x5076E8], rows to [0x5076EC..0x5076F0], insert sorted
FUN_0043D247(surface)
    for every run: pixel = (pixel & 0xF7DF) >> 1      (16 bpp / 800x600)
                   pixel = (pixel & 0x7BDF) >> 1      (otherwise)
```

Producer, in the world render pass `FUN_00426384`, gated by the `shadows`
config word `0x81D620`:

```text
for each active entity in the location : FUN_0043C040(body frame mask)
                                        + FUN_00424ED0 (one mask per equipped overlay)
for each projectile in 0x80BD50        : FUN_0043C040(mask)
FUN_0043D247(surfaceScreen)
```

Mask data: GRAPH frame records are `{u32 shadow_offset, u32 sprite_offset}` at
`human block + 0x1728 + frame_id*8` and `object block + 0x0728 + frame_id*8`
(payload base `block + 0x4D28` / `block + 0x3228`). The 8-byte anchor record
(`human block + 0x2F28 + action*0x300 + direction*0x60 + fi*8`,
`object block + 0x1728 + row*0x480 + direction*0x90 + fi*8`) carries **two**
`(gx, gy)` pairs: `+0/+2` is the shadow anchor, `+4/+6` the sprite anchor.
Shipped example: human base 0 frame 0 = `43x85` at offset 0 with a `107x22`
shadow at offset 2167; object 0 frame 0 has a `237x43` mask.

**CHECK:** the anchor pairs genuinely differ (e.g. human base 0 walk dir 0
frame 0: shadow `(-83,-15)` vs sprite `(-25,-75)`), so the shadow is a real
ground offset, not an under-sprite overlay. Never reuse the sprite anchor for
the shadow.

Implemented in `src/render/spans.rs` (`add_mask`, `darken`) and
`src/res/shadows.rs` (`Res::add_actor_shadow`), wired in
`app/world_draw.rs::draw_detail`. Deviations: plain RGBA8 channel halving
instead of the 16-bit word mask, no run budget (per-frame allocation), no
dirty-rectangle reuse (full-frame redraw), projectile shadows not ported
(INTR sprites carry no dedicated mask).

---

# 29. `CLIENT.EXE`

`CLIENT.EXE` (939008 bytes, PE32/i386, `machine 0x14C`, `e_lfanew 0x80`) is a
**separate build** from `konung.res` (1586688 bytes). It contains the same
`KONUNG.` / `GAME.` filename strings and the same class of game code, but is
smaller and is an earlier/alternate executable.

Every virtual address in this document (`0x438A78`, `0x411DE4`, `0x43DA1F`,
`FUN_0043adf0`, all table VAs, …) is **`konung.res`-specific** and must not be
assumed to apply to `CLIENT.EXE`. The remake is driven by `konung.res` only.
`CLIENT.EXE` is not referenced anywhere in this contract; if a future task needs
it, re-derive its addresses independently.

---

# 30. Dialogue / quest system (`QUEST_R.RES`)

**Status: ORIGINAL-CODE VERIFIED (structure), DATA-VERIFIED (text).**

`konung.res` is code only; dialogue, conditions and actions live in the separate
`QUEST_R.RES` (337800 bytes), loaded by `FUN_0043a870` (`0x43A870`) into seven
fixed buffers. Full trace: `DIALOG_SYSTEM.md`.

- File blocks (file offset -> buffer VA -> length): B1 `0..128000 -> 0x5C2D90`
  (node/option records, stride `0x10`), B2 `128000..128600 -> 0x5E2190`
  (`u16[300]`, `dialogue_id -> first node`), B3 `128600..192600 -> 0x5E23E8`
  (CP866 text), B4 `192600..204600 -> 0x5F1DE8` (condition programs), B5
  `204600..216600 -> 0x5F4CC8` (action programs), B6 `216600..336600 -> 0x5F7BA8`
  (text-offset lists), B7 `336600..337800 -> 0x615068` (`u32[300]` flag slots).
  The seven lengths sum to the file size.
- Entry point: clicking an NPC whose `Entity+0xD0` (`dialogue_id`) is not `0xFF`
  calls `FUN_00435290` (`0x435290`, from the gameplay driver `FUN_0042260C`);
  `dialogue_id` indexes B2 for the first node, and the archetype (`Entity+0x04`)
  selects the window template id (`0x0C..0x13`).
- Node/option record (`0x10` bytes): `+0x00` u32 B6 line index (`0xFFFFFFFF`
  none), `+0x04` next node (`-1` fall back, `-2` end), `+0x06` condition id,
  `+0x0A` action id. Headers use `+0x0C`(low)=unconditional /`+0x0E`(high)=
  conditional entry counts, and `+0x0A == -2` as the branch marker.
- Programs (`B4`/`B5`): `u16[]` ended by `0xFFFF`; `0x2000` = run script opcode
  `FUN_0043159C`, `0x4000` = condition group separator, `0x8000` = set/negate B7
  flag `operand`.
- Text: `FUN_00435188` joins B6-listed B3 fragments (CP866) with a single space
  before letter/`-`/digit starts; `B3` is an offset pool, not one string.
- Interface: drawn by `FUN_0042CBD0(param 0)` (case 1), which tiles **`INTR[5]`**
  (100x100 stone, `FUN_00430430`), then blits **`INTR[1]`** (the ~3%-opaque ornate
  border) and **`INTR[0]`** (divider band) at per-resolution tables
  `0x4632C4..0x4632DC`, and draws text with the small font `0x58BC34` plus the
  selected option with the highlight font `0x58B8F0`; `FUN_0042BD8C` advances
  15 px per wrapped line. The `f7` (conditional) entries are the NPC statement and
  the `f6` (unconditional) entries the selectable options (`0x788FB4` / `0x788DB4`).
  `FUN_0042A13C` is sound playback (not a window opener); the `0x0C..0x13` value is
  an NPC voice id.
- Script opcodes (`op >= 0x112`) dispatch through the op-indexed pointer table at
  `0x465BD4` (real base `0x46601C`); `0x112..0x16E` are the shipped handlers,
  catalogued in `src/script.rs` (reference `QUEST_OPCODES.md`). **Trade/loot = `0x113`** (`FUN_00431E2C`):
  loads both inventories/money and opens exchange interface mode 7
  (`FUN_00430D6C(0)`); full pricing/transaction/UI trace in `TRADE_SYSTEM.md`.
  `B7` slot = `{u8 flags (bit 0x80 active), u8 ?, u16 @+2 = B6 journal line}`.
- Remake: `src/quest.rs` (decoder) + `src/dialog.rs` (click NPC -> dialog,
  hover/click options) + `src/script.rs` (opcode catalogue/program helpers +
  low-family dispatch) + `src/app/dialogue_exec.rs` (page/selection
  orchestration) + `src/app/dialogue_ops.rs` (`QuestContext::exec_op`, every
  high handler and all low families); ABI `KIND_QUEST = 13`; manifest field
  `quest`; report `DIALOG_SYSTEM.md` / `QUEST_OPCODES.md`.
  Conditions and actions are **executed** (`FUN_00434FE8` / `FUN_00434BB8`):
  every low family `0x00..0x111` and high opcode `0x112..0x16E` is dispatched;
  `0x16A` is implemented as party gear durability repair/cap (`npc.smithing*10`, 30% excess reduction). `Both`-kind
  opcodes execute during condition evaluation. The recruit opcode `0x123` adds
  the dialogue NPC to the reserved party **group 0** (hud panel + selection array
  `0x81A814`), `0x14C` dismisses it; trade (`0x113`) opens the exchange
  (`src/trade.rs`, `trade_screen.rs`, `app/trade.rs`). Quest additions
  (player `+0xD0/+0xD2/+0xD3`, settlement money pool/target base, group
  activity/visited) persist in `save.rs::SaveExtra` (save v2; v1 defaults). Quest file: the executable opens `QUEST_R.RES`; the install's
  `Quest_r.res` is its case-insensitive alias holding the real script, while the
  separate `QUESTR.RES` is a placeholder (dialogues 97/96 are `"???"` stubs), so
  `make_manifest.py` prefers `QUEST_R.RES`/`Quest_r.res` over `QUESTR.RES`.
- UNKNOWN: display-header `B1 +0x06`; `B7` slot byte `+0x01`; per-op argument
  semantics where a handler reads no pushed argument.

---

# 31. Sound system (`SOUNDS.RES`)

**Status: ORIGINAL-CODE VERIFIED (format/engine).**

- Files: `SOUNDS.RES` (21 981 240 B, 391 clips) and `Sounds_.res` (13 314 264 B,
  244 clips) — same format, partly different content; the game opens
  `SOUNDS.RES` (`FUN_004392B4`).
- Container: 8-byte header (`magic 00 00 00 01` + u32) + `1000 x {u32 offset, u32
  size}` index at offset 8; data base **file offset 8008**; clips are raw signed
  **16-bit little-endian mono PCM** (`frames = size/2`).
- Engine: DirectSound (`DSOUND.dll`). `FUN_00429C0C` creates three secondary
  buffers at **22050 / 19050 / 25050 Hz** (mono 16-bit) = pitch variants;
  `FUN_0042A13C(id, x, y, z)` copies a preloaded clip (`0x81A990[id]`) and plays
  it; `FUN_0042987C` shuts down. NPC voice ids are `0x0C..0x13`.
- Remake: `src/sound.rs` (decoder) + `sound/events.rs` (verified id
  tables); ABI `KIND_SOUNDS = 14`; manifest field `sounds`; a one-frame
  `PlayableApp.sounds`/`Frontend.sounds` queue drained by `www/app.js` into
  WebAudio. Report `SOUND_SYSTEM.md`.
- Event ids (22 direct `FUN_0042A13C` call sites): UI/menu/inventory click `4`
  (`0x43687e`, `0x435dca`, `0x422914`, `0x4242e6`, `0x424362`, `0x42146c`,
  `0x421568`, `0x4216f0`, `0x422480`, `0x41e8a8`); item use `9`
  (`0x41e0e9`); class-11 map/scroll item `10` (`0x414de4`); main equip `11`
  (`1` for class 2) and additional equip `0` (`0x41e680`/`0x41e872`); dodge
  `6` (`0x421e1c`); dialogue voice `0x0C..0x13` (`0x43538f`); actor action
  table (`0x4144fd`/`0x414775`, tables `0x4146e8`/`0x414480`) and per-location
  ambience `0x100 + (loc-1)*8 + rand%8` (`0x4365d4`, 2%/frame).
- UNKNOWN: header u32; exact 19050/25050 pitch selection per event (playback
  currently uses the 22050 base rate; positional volume/pan deferred).

---

# 32. Game.N WorldObjects / collectable herbs

**Status: ORIGINAL-CODE VERIFIED (load/pickup/respawn) + DATA-VERIFIED.**

- Table: `Game.N +0x21900`, 500 � `0x40`, loaded verbatim by `FUN_0043A870` into
  `0x77DEB4`. Layout:
  `+00/+04` i32 activation timestamps, `+08/+0A` u16 runtime cell,
  `+0C` u16 sprite id, `+0E` u16 source index, `+10` i16 money, `+12` u16
  location, `+14..+3F` 22 � u16 item-instance ids (indexes into the
  `Game.N +0` 8192 � 16 item table, not class/subtype).
- Entry: `FUN_0043ADF0` (`0x43BC05..0x43BD89`) accepts a record when
  `+12 == current_location` and (`+00 >= 0` or `+00 != +04` or
  `now + +00 >= 0x1518`), copies it to the runtime pool `0x785BB4`, sets cell
  bit `0x2000`, resolves the sprite with `FUN_00438CC8`, projects the cell with
  `FUN_00438A78` and stores the centered top-left anchor in runtime `+00/+04`.
- Sprite (`FUN_00438CC8`): only a record with exactly one item (`+14 != 0`,
  `+16 == 0`) and no money (`+10 == 0`) uses that item's world-sprite field;
  otherwise `0xA3`. Classes: `0..=5` prototype/world sprite at `+3C`,
  `6..=9|11` class record `+4`, class 12 = `0x99`, class 10/unknown = `0xA3`.
  `???????? ????` = class 9 subtype 1 ? INTR **160**; `????? ??????` = class 9
  subtype 2 ? INTR **168**.
- Render: `FUN_0042532C` marker pass after terrain, depth-sorted entities and
  party rings, in x-then-y order; draws `+0C` from the INTR table at
  `EntityProjection(cell) - sprite_size/2 + camera`.
- Pickup: a **right-click** sets `Entity+0x03` bit `0x40` (left-click clears
  it). On interaction arrival at a `0x2000` cell, `FUN_00421B50` calls
  `FUN_004110EC`: add `abs(+10)` money to `Entity+0x34`, then transfer every
  `+14` item with weight limit `(endurance_base*1000)/3 + 10000`, 32 inventory
  slots, insert at slot 0 and shift, no stacking. Full/overweight opens the
  exchange UI and shows the original messages. Walk-over without bit `0x40`
  never collects.
- Respawn (`FUN_0041093C`): only when the source record had exactly one item of
  class 9 subtype 1 or 2. It allocates a free item instance (`FUN_00438884`,
  first signed class byte `< 0`, indexes 1..8191), copies the 16-byte collected
  item, writes the new id to source `+14`, and stores `-frame` in `+00/+04`.
  The record respawns on a later map load when `now - deactivated >= 0x1518`
  (5400 frames = 324 s at 60 ms). Every other emptied record gets `+12 = 0`
  (permanent).
- Data: Game.1 has 95 herb nodes � loc 3: 19 `???????? ????`; loc 4: 1 nut +
  4 `????? ??????`; loc 27: 5 roots; 24/33/etc. mix; many locations have none.
  Game.0/1/2 differ by only 2�3 nodes.
- Remake: `src/world_objects.rs` (parser/activation), `src/app/world_items.rs`
  (rebuild/pickup/respawn), `src/app/world_draw.rs` (marker pass),
  `item.rs::ItemInfo::world_sprite`. State is session-local; save/load and the
  exchange UI are out of scope.

---

# 33. Melee combat, death and corpse loot

**Status: ORIGINAL-CODE VERIFIED** for the formulas/constants below
(`konung.res` `d698c4f231489a372cc25212ab2819720a36d7ce5307a045db684135cf679810`).
Evidence: REA decompiles/reads `0x41b060`, `0x41b42c`, `0x41aea8`, `0x41ab7c`,
`0x41c3cc`, `0x41c1e8`, `0x4123bc`, `0x413de4`, `0x41d048`, `0x41bd3c`,
`0x418480`, `0x4179c8`, plus the DGROUP reads at `0x450020`, `0x4500d0`,
`0x450db4`, `0x462798`.

## 33.1 Attack order (`FUN_0042260C`)

A right-click on a non-dialogue actor (different group from the leader) writes
for every selected party unit (`0x81A814`) that has `Entity+0x03 & 4` (battle
stance):

- order `0x44 'D'` when `+0x12 == 1 && +0x68 != 0 && +0xCC != 0`;
- otherwise order `0x41 'A'`;
- the clicked actor index is stored in `Entity+0x06`; queue terminator `+0x14 =
  0xFF`. No destination is written for actor-target orders, and **no move is
  issued** when no unit is in battle stance.

## 33.2 Hit chance (`FUN_0041B060`, `FUN_0041B42C`)

`rand()%101`; a hit requires `reaction >= roll`.

```text
monster (Entity+0x04 & 0x40): reaction = round(50 + dexterity_base*0.25)
human melee:  reaction = round(100 - (1.5 + melee*0.00015) / (100 - dex_base*0.5))
human archery (weapon slot 1): same with archery*0.00015 and the bow-range term
```

## 33.3 Attack damage (`FUN_0041AEA8`)

```text
monster:            round(STR_base * HP * 0.04 * 0.0625)  = STR*HP/400
human unarmed:      round(STR_base * HP * 0.02 * 0.0625)  = STR*HP/800
human armed (0/2):  weapon_value + round(STR_base*HP/400)
```

`weapon_value` is the `0x462798` modifier value when `instance+0x02` matches the
target archetype, else the prototype rating (`0x459B7C + subtype*0x40`).
Poison added on hit is the weapon instance `+0x0C`, or `Entity+0x50` for a
monster attacker.

## 33.4 Damage application (`FUN_0041C3CC`)

```text
armor = base_armor (+0x4E) + sum(rating(equipment[3..5]))
if facing_table[defender.facing*8 + attacker.facing] != 0
   and rand()%101 <= round(defender.dex_base*0.5): miss (dodge)
temp  = max(0, incoming*0.7 - armor)
final = round(16 * (1 + (incoming*0.3 + temp) / endurance_base))
```

A hit is lethal when `final >= hp`. On a landed hit:

```text
defender.periodic_damage (+0x50) += attacker poison
armor wear ratio = 1 if armor == 0 else min(incoming/armor, 1)
for slot 3..=5: durability(+0x04) -= ratio * item_rating * 0.001
attacker weapon: decrement +0x0C counter; durability -= incoming * 0.0001
```

A broken instance gets class byte `0xFF` and its equipment slot is cleared.

## 33.5 Animation / death

Melee resolves on the last attack-animation frame (`FUN_00413DE4` returns 0).
Death rows are `3` (battle) / `9` (peace); object rows `6..=11` alias `0..=5`.
On the second-to-last death frame the collision cell is freed (`& 0xF000`) and
the final frame is held.

## 33.6 Periodic damage (`FUN_0041D048`, `FUN_0041BD3C`)

Every 16 logic frames (`_DAT_0081D640 & 0xF == 0`): `hp -= periodic_damage`;
`hp < 1` enters the death row and spawns corpse loot with
`(bonus=0, killer_level=leader_level)`.

## 33.7 Corpse loot (`FUN_00418480`)

- money = `Entity+0x34` + (`rand()%bonus` when `bonus >= 2`);
- up to 22 non-zero `Entity+0x72` inventory ids;
- if `Entity+0x52 != 0`: always add class-9 subtype-0 `???????? ????`, current
  durability 1.0, instance `+0x0C = Entity+0x52`;
- generated item only when `killer_level != 0 && rand()%101 > 85` (melee passes
  `(0,0)`):

```text
86:      class 0/1/2, subtype = rand%(level/5+1) + (rand%9)*3;
         class = 2 if subtype%9<3, 0 if <6, else 1
87:      class 3/5/4, subtype = rand%(level/5+1) + (rand%3)*11 + 0x22/0x25/0x28
88..94:  class 8, subtype = rand%(level/5+1) + 6
95..96:  class 7, subtype = rand%(level/5+1) + 3
97..100: class 6, subtype = rand%(level/5+1)
```

The generated quality roll (`FUN_00418480` writes it to instance `+0x01`, not the `+0x02` damage modifier) uses tier `FUN_00417144`: level `<4 -> 3`,
`<7 -> 6`, `<10 -> 8`, else `10`; quality `= rand()%tier + (rand%5)*10`.

The corpse is the runtime WorldObject marker at the death cell; the existing
right-click arrival pickup transfers money and items.

## 33.8 Remake

`src/combat.rs` (rules/RNG), `src/combat/loot.rs`, `src/app/combat.rs`
(orchestration/input), `item.rs`/`res.rs` (instance modifier + durability).
Ranged projectile combat (action row 4) is deferred.

## 33.9 Ranged combat / projectiles

**Status: ORIGINAL-CODE VERIFIED** (`FUN_004123BC` case 4, `FUN_0041BF44`,
`FUN_00420F30`, `FUN_0041AC40`, `FUN_0041C1E8`, `FUN_0043E4EA`,
`FUN_004245B4`; constants `0x45013c`, `0x450144`, tables `0x45ACD0`,
`0x45ACD1`, `0x4580A0`, `0x4580A1`, `0x459B7C`).

- Bow = `Equipment[1]` (`Entity+0x68`); active weapon `Entity+0x12 == 1`.
- Ammo = `Entity+0xCC` special item; class 12 stack; quantity `+0x04 u32`.
  Spawn consumes one; at zero the slot and instance are cleared.
- Each shot also subtracts `0.1` from the bow instance `+0x04`; at `<=0` the bow
  breaks (`class=0xFF`, `Equipment[1]=0`).
- Action row `4` fires on animation frame `count-6`; no approach/range check.

Spawn (`FUN_0041BF44`):

```text
start = EntityProjection(render_cell) + muzzle[facing]
target = EntityProjection(order cell); target.y -= 30
fx = |dx|*0.25 + 0.001; fy = |dy|*0.25 + 0.001
if fx < fy: v = (dx/fy, dy/fy) else v = (dx/fx, dy/fx)
life = prototype byte +0x08 of the bow subtype
sprite index = 184 + 2*facing (+1 on odd world frames)
draw offset = (0x4580A0[facing*2], 0x4580A1[facing*2])
```

Muzzle tables `0x45ACD0` / `0x45ACD1` (signed bytes):

```text
x: -70 -48 12 68 80 52 -8 -60
y: -65 -99 -85 -90 -58 -26 -10 -31
```

Damage (`FUN_0041AC40` / `FUN_0041C1E8`):

```text
base = bow modifier value (instance +0x02) if its target archetype matches
       else bow prototype rating (0x459B7C + subtype*0x40)
raw = base * bow_rating
if rand()%101 <= round(target.dex_base*0.5): dodge (arrow still removed)
armor = base_armor + S rating(equipment[3..5])
temp  = max(0, raw*0.7 - armor)
final = round(16*(1 + (raw*0.3 + temp)/endurance_base))
hp -= final; poison = bow +0x0C
armor wear ratio = min(raw/armor,1); slots 3..5 durability -= ratio*rating*0.001
```

Kill (`FUN_00420F30`): death rows 3/9, owner money `+= target_power/4`,
owner archery mastery `+= target_power`, corpse loot bonus = `target_power`,
killer level = owner level.

Flight: eight sub-steps per 60 ms frame; blocked collision cell removes the
arrow. Actor cells damage any living non-owner (friendly fire included).

Prop branch: a right-click prop writes target id `-(prop_index+1)` and the
clicked cell, only when ammo instance `+0x02 == 0`. On a blocked cell the arrow
is removed; if the point is opaque on the prop sprite (`FUN_0043E4EA`), then
with `rand()%100 < 21` settlement target entry `+1=3`, `+6=1`,
settlement `+0x4B2 |= 0x15`, and the settlement group (`+0x02`) is woken
(`FUN_004245B4`: activity + clear queues).

**Arrow recovery (verified negative).** The fired arrow is consumed at spawn
and never re-added by the projectile, damage or corpse-loot code. Lootable
arrows come from the dead actor's own inventory: `FUN_00418480` copies it.
`Game.1` entity 40 (loc 4, group 4) carries item 102 = 30 �?????????? ??????�;
13 shipped mobs carry class-12 stacks.

---

# 34. Player inventory, alchemy and potion use

**Status: ORIGINAL-CODE VERIFIED** for the routines below (`konung.res`).

## 34.1 Containers

```text
Entity+0x66..+0x71  equipment[6]
Entity+0x72..+0xB1  inventory[32]
Entity+0xB2..+0xBB  additional[5]
Entity+0xCC         special
```

`FUN_0041E450` places a main-slot item only when `item.class == slot` (`0..5`)
and the requirement predicate `FUN_004182D4` passes; the displaced item is
inserted at inventory slot 0 (`FUN_0042462C`). `FUN_0041E6BC` gates additional
slots: class `6 -> 0x0E`, `7 -> 0x0F/0x10`, `8 -> 0x11/0x12`. `FUN_0041E8A8`
is the `0x13` special-slot pick-up/put-down path; its search order is special,
main, additional, inventory.

## 34.2 Weight

`FUN_0041B7E4` sums all four containers; class-12 instances use
`prototype.weight * (+0x04 u32 quantity)`. Capacity is
`(endurance_base*1000)/3 + 10000`; failure is strict `capacity < weight`.
Automatic stacking is class-12 only (the original has no generic stack merge).

## 34.3 Player alchemy

`FUN_0041DAB0` with a held class-9 item and a class-9 target calls
`FUN_0041BB70`. The recipe table is 33 records of 6 bytes at `0x450DF4`:

```text
{subtype_a, subtype_b, out_a, out_b, skill_gain, op}
```

The held item becomes `out_a`; the target becomes `out_b` (`3` empty jar,
`0xFF` consumed). Quality (`+0x04`) is computed by `op`:

```text
0: healing*0.01
1: quality + healing*0.01
2: quality + target_quality
3: min(quality, target_quality)
4: target_quality + healing*0.01
clamp 15.0
```

`healing` (`Entity+0x48`) gains `skill_gain`, capped 500. No row:
`out_a = 4` («Непонятная смесь»), target `class = 0xFF`.

## 34.4 Potion use (`FUN_0041DAB0`)

Right-click use changes the item subtype to `3` (empty jar) in place. Effects by
class-9 subtype:

```text
4  HP /= 2 (min 1)
5  HP = min(1600, round(quality*160 + HP))
6  periodic_damage += round(quality)
7  requires a target item; clears target +0x02 when quality >= 10 and class 12
8  periodic_damage = max(0, round(quality*10 - base_armor))
9  drunk buff: timer = local*30; DEX -= local*3; CHA/STR/END += local
10 buff: timer = local*30; DEX/STR/END += local; HP heal
11 timer = local*30; effect id = round(local*0.1)
12 requires local >= 6: available XP += (local-5)*100; timer = local*60;
   STR -= local*5 (min 1); HP -= local*160 (min 160 if CHA < 160)
13 HP = 1600; revive when dead
```

Temporary effects save DEX/CHA/STR/END at `+0xBC..+0xC2` and restore them when
`+0xCE` reaches zero. The remake implements these in `inventory::use_item` /
`StatOp` and ticks at the 60 ms logic rate.

## 34.5 Ground drop and companion owners

`FUN_004138C8` appends an item id to the per-cell runtime marker pool
(22 slots) and resolves the sprite. The remake ports this as
`PlayableApp::add_ground_item`. The character screen edits any owner
(`_81D560`); clicking a party portrait while the screen is open switches the
owner. Settlement production (`FUN_004175FC`, `FUN_004179C8`, `FUN_00417F84`)
is background economy and is not part of this scope.

## 35. Settings, `KONUNG.CFG`, lighting and day/night

### 35.1 `KONUNG.CFG`

`KONUNG.CFG` is exactly 28 bytes: seven little-endian `u32` fields.

| Offset | Global | Meaning |
|---|---|---|
| `+00` | `0x81D644` | video mode: `0x111` = 640x480, `0x114` = 800x600 |
| `+04` | `0x81D5D4` | transparency |
| `+08` | `0x81D5EC` | day/night |
| `+0C` | `0x81D604` | light effects |
| `+10` | `0x81D60C` | music |
| `+14` | `0x81D620` | shadows |
| `+18` | `0x81D62C` | sound |

A missing file leaves every flag `1` and video `0x111`; an invalid video mode
is forced to `0x111`. The remake defaults to `0x114` (browser frontend starts at
800x600) and stores the same 28 bytes as base64 in
`localStorage["openkonung.cfg"]`.

### 35.2 Settings screen

`FUN_00436214` enters settings at `0x436C79`, snapshotting the globals into
`0x831C54..0x831C6C`. The `0x46257C` table has 10 records, each
`{x1,y1,x2,y2}` for 640 then 800, **inclusive** right/bottom edges:

| # | Control | 640 | 800 |
|---|---|---|---|
| 0 | video 640 | `(351,112,381,142)` | `(435,144,475,184)` |
| 1 | video 800 | `(474,112,504,142)` | `(588,144,628,184)` |
| 2 | transparency | `(413,166,443,196)` | `(513,206,553,246)` |
| 3 | shadows | `(413,202,443,232)` | `(513,250,553,290)` |
| 4 | day/night | `(413,236,443,266)` | `(513,294,553,334)` |
| 5 | light effects | `(413,272,443,302)` | `(513,338,553,378)` |
| 6 | music | `(413,306,443,336)` | `(513,382,553,422)` |
| 7 | sound | `(413,342,443,372)` | `(513,426,553,466)` |
| 8 | Ok | `(250,420,319,480)` | `(300,510,399,600)` |
| 9 | Cancel | `(320,420,390,480)` | `(400,510,499,600)` |

Controls 0/1 set the video choice; 2..7 toggle live values exactly
`0 -> 1`, non-zero -> `0`. Ok applies the video rebuild and writes the file;
Cancel (`0x437C39`) restores `0x831C54..0x831C6C`. `FUN_0043060C` centers the
13x13 `MENU#2` marker in each enabled control.

### 35.3 `LIGHTS.RES`

`FUN_004392B4` opens `LIGHTS.RES` and reads it in file order:

| File offset | Size | Destination | Meaning |
|---|---|---|---|
| `0x0000` | `0x1000` | `0x5A0010` | 64 x 8x8 masks |
| `0x1000` | `0x0AC0` | `0x615518` | one 64x43 mask |
| `0x1AC0` | `19 x 0x1C80` | `0x5A1010` | nineteen 114x64 light tiles |

The shipped file is 145472 bytes; only 145088 are read (the last 384 bytes are
ignored). Tile pixels are one byte `0..31` (`31` = full light).

### 35.4 `LIGHTS.N`

`FUN_0043ADF0` opens `LIGHTS.<location>` and reads exactly 64000 bytes:

| File offset | Size | Destination | Meaning |
|---|---|---|---|
| `0x0000` | `0x3200` | `0x650318` | low map, 160 rows x 80 columns |
| `0x3200` | `0xC800` | `0x643B18` | high map, 320 rows x 160 columns |

A missing/short map zero-fills both. Low index is `col + row*0x50`; a non-zero
value selects tile `value-1` from `0x5A1010`. High-map index is
`Entity+0x0A + Entity+0x08*0xA0`; non-zero means the entity cell is lit and uses
the unshaded palette.

### 35.5 Ambient / render gates

`FUN_00428FE0` computes the ambient palette delta from
`world_frame % 0x5460` (21600) with thresholds
`0,1800,1830,3570,3600,5400,8100,11700,11880,11910,16155,16200,18900`. The
returned `u32` packs signed BGR bytes. A non-zero `0x82D168` marks night.
`FUN_0042532C` shades terrain/entities only when
`night != 0 && light_effects (0x81D604) != 0`; `day_night (0x81D5EC) == 0`
makes the ambient delta zero. `0x81D5DC != 0` selects the location override
table `0x45ACE0`, whose shipped entries are `0x00FFFFFF` except the last.

## 36. Original movies and playback state

### 36.1 Container / codec

The CD ships `Konung\Video\*.flm`, but the extension is misleading: every file
is a RIFF AVI (`RIFF....AVI LIST`) with `vids`/`IV50` (Intel Indeo 5 video,
640x360, 12 fps) and PCM audio (`auds`; sample rates vary, e.g. poster
44.1 kHz, short clips 22.05 kHz). The files are not playable by browser
`<video>`, so `tools/transcode_videos.py` produces `www/video/<stem>.mp4`
(H.264/AAC, 640x360, 12 fps) for the remake. The original `.flm` remain the
input of record and are never shipped to `www`.

| Clip | Frames | Duration | Trigger |
|---|---:|---:|---|
| `poster.flm` | 2793 | 232.75 s | startup state `9`, then menu page `7` |
| `win.flm` | 4261 | 355.08 s | WorldObject source index `0` pickup, then menu |
| `gameover.flm` | 173 | 14.42 s | leader game-over flag, then menu |
| `defeat<loc>.flm` | 101 | 8.42 s | settlement defeat, then gameplay |
| `credits.flm` | 617 | 51.42 s | CD launcher only (not the game) |

The shipped `defeat` set is locations
`1,4,7,11,14,16,19,22,24,27,29,33,35,37,39`. A missing/short clip is a normal
`PlayAVI` failure; the state still becomes `_81D6B8`, so playback is skipped.

### 36.2 Trigger paths

- **Startup:** `WinMain` sets state `9`; `0x438021` sets `_81D5F8 = 0`,
  `_81D6B8 = 7`, and calls `FUN_0042ADE4("POSTER.FLM")`.
- **Game over:** gameplay case `0x43653F` tests `leader[+4] & 0x80`; when set
  it sets menu page `7`, `_81D5F8 = 0`, and plays `GAMEOVER.FLM`. The flag is
  the `OR byte [EAX+4],0x80` path in the actor/group/death code.
- **Victory:** `FUN_004110EC` starts with
  `FUN_00410E10(cell)` and tests the runtime marker `+0x0E` (source table
  index). Index `0` sets `_81D6B8 = 7` and plays `WIN.FLM`; ordinary pickup
  continues only for non-zero sources.
- **Settlement defeat:** in `FUN_0041D048` (periodic pass, every 16 frames),
  a settlement group engaged with the party (`Group+0x1D == 1`, party inside
  rectangle A) and
  `Settlement+0x4B2 & 0x80 == 0` compares total party health with the
  group's live level `<3` defenders. If
  `party_hp / defender_hp >= _DAT_004501A8` (`2.0`), it sets
  `+0x4B2 |= 0x80`, stops the group, sets `_81D6B8 = 0`, builds
  `"DEFEAT" + decimal(location) + ".FLM"`, and plays it. Movie end therefore
  resumes state `0` (gameplay), not the menu.
- `FUN_0042ADE4` builds `<CD-root>\VIDEO\<name>`, stops music
  (`FUN_00429714`), calls `PlayAVI.dll::playAVIFile`, restores the window, and
  sets `_81D5AC = _81D6B8`. The playavi2 form forwards click/key close events,
  so the remake treats click/Escape/Skip as an early end.

### 36.3 Remake transport

`video.rs` holds one pending `(VideoClip, VideoAfter)`; `abi/video.rs` exposes
`start_intro_video`, `video_pending`, `video_name_ptr/len`, `video_finish` and
debug selectors. While a clip is pending, `step` and gameplay input pause. The
shell resolves the clip from `pack_video_*` first and falls back to
`manifest.videos[<stem>].file`, pauses music, and plays it through a full-screen
`<video>`; completion/error/skip calls `video_finish`, which applies the
original post-state (`Menu` or `Gameplay`). `server.py` implements HTTP Range
responses so the fallback MP4s can seek.

Pack format `3` adds `videos: Vec<VideoBlob>`; the pack build id includes their
names and bytes. `make_manifest.py` also lists all 19 fallback URLs and folds
their hashes into the page build id.


