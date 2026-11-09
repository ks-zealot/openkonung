# Konung: Game.2 groups, NPC initialization, and location spawning

Investigation date: 2026-10-13. Static analysis through REA MCP, Ghidra 12.1.4.
All addresses below are virtual addresses in the original x86 PE at image base
`0x00400000`, not offsets into Game.2. Function names are analyst descriptions;
the executable does not supply these source names. Pseudocode is a reconstruction
from Ghidra output checked against relevant instructions, not recovered source.

## Results

**ORIGINAL-CODE VERIFIED:** `0x43A870` bulk-loads the Game variant's groups and
entities. `0x43ADF0` loads Konung.N and activates matching groups. It traverses
`Group+1C`, conditionally randomizes positions through `0x411DE4`, registers
occupancy, and rebuilds animation state. Existing authored NPC records are not
regenerated merely because their coordinates are zero.

Three counts must be distinguished:

- `+18`: desired population in the replenishment path; also a party-size limit
  for the protagonist's group.
- `+19`: reserved capacity / upper population limit.
- `+1C`: actual contiguous entity count used for traversal and appending.

The original uses load mode `0` for new games/location travel and mode `1`
for save restoration. The former may reposition NPCs and replenish groups;
the latter preserves placement and animation frame state.

Two corrections to existing notes are especially significant: the random-placement
routine has **no explicit coordinate bounds check**, and its caller does **not**
use `(0,0)` as the placement predicate. The applicable group flags and load mode
are the predicate.

## Artifact identity and evidence

Read-only binary inspections and hashing:

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| konung.res | 1,586,688 | `d698c4f231489a372cc25212ab2819720a36d7ce5307a045db684135cf679810` |
| Game.2 | 709,822 (`0xAD4BE`) | `a95cc1c72f495f2ed661f71ab511f91a9980b03a1abd528ecca09a896dac58af` |

Despite its suffix, konung.res is an MZ/PE32 x86 executable. Its PE header is at
file offset `0x80`. The hashes were checked again after analysis and matched.

Evidence files:

- [Main REA evidence](KONUNG_GAME2_NPC_REA_EVIDENCE_2026-10-13.json):
  36 complete retained records exported by REA (21,985,175 bytes).
- [Population timer evidence](KONUNG_GAME2_NPC_TIMER_REA_EVIDENCE_2026-10-13.json):
  independently retained inline REA records from the parallel read-only timer trace.
  This is a saved collection of exact tool responses, not a REA export-format bundle.

Evidence keys used below:

| Key | Function / observation | REA Evidence ID |
|---|---|---|
| L | Game variant loader `0x43A870` | `ev_169df2774c3126359417e2ea8dc242a0f5925a89764b9ce5975b6a18beb00edb` |
| M | Location setup `0x43ADF0` | `ev_65a419123013a5703f0b53ff5642ba1342e7e9291c059fbbfaff8ed6da1fcdf3` |
| P | Random placement `0x411DE4` | `ev_64cf22ee3e5f9803c7c1e8afa0360cdde63c6e8a1cb8a82a6714301c2eb0d2ed` |
| C | Population constructor `0x420A8C` | `ev_ba536052efa8533b65e830d35c52932d37f87e6397885cb8d340bdc7f7d92641` |
| I | Reset/rebuild entity action `0x41435C` | `ev_9e39b69722bc72cce3d24594d229abf9c4a3072e301a36ed0fec467d2a9961fe` |
| R | Restore graphics pointers `0x414078` | `ev_4f9a8565b65ae7f71cceccf2f4b0affd51c4228a8ad528b1c1ddb5128c4e1448` |
| S | Saved-game loader `0x4246C0` | `ev_4da5d45f2d179939de63e687ef9aa8f8fc96ad3b2c75d9db01d2f3af1372e2a7` |
| D | Main dispatch `0x436214` | `ev_38969f46033b1a15eabd51e5df0a909e6a302338a4e64cdd558d1b7a6008b01b` |
| T | Population bookkeeping `0x41BD3C` | `ev_4e53226f8c2da6ac1cc3fdaf4ec46e4e77202d9c8d28518ac8c0a58db03d20a1` |
| TA | Bookkeeping assembly | `ev_60e1c7208b742d8cc45b9c8db788e047a5441a766646d2e0c03c62f3bf176254` |
| U | Periodic group dispatch `0x41D048` | `ev_2b491792cd2637b8a79b297e9ade6b34cc74731fb3697c238f54729b91aed0ea` |
| RNG | RNG `0x43EA46` | `ev_653b2db1077fd74d2753c769d0e55a4e5c8ebf21dced02b42031607759b7fad1` |
| F | Coordinate conversion `0x438A78` | `ev_370faf2e1e15bd07ebf1e07bfda77a689585e4c0c3e181e55a8b155b4057e89b` |
| Party | Party arrival placement `0x4118DC` | `ev_bc0916ae6cfad407c8b1c1ee1ae7ef6822131aaa2a6cb7a36f5586b32dcea042` |
| Limit | Party size calculation `0x411078` | `ev_3c5003591ec995c7c79435860246a905d18209ad13a103bc8b356cb622df57ed` |

The main dispatch dossier exceeded MCP's transport budget. It was retained intact
and inspected through native pseudocode and assembly views. Its analyzed body has
six disjoint ranges, not one continuous function span; high-pcode coverage is
reported as truncated. The caller observations here use the returned assembly and
pseudocode, not an assumption of complete high-pcode data flow.

## 1. File-loading path

**ORIGINAL-CODE VERIFIED (L).**

`0x43A870..0x43ABF9` constructs:

```c
path = resource_directory_at_0x81CD30
     + "GAME."
     + decimal((int8_t)memory[0x81D532]);
fd = open_binary_read(path);               // 0x43ECA7(path, 0x200)
if (fd == -1) { error(...); return 0; }
```

Thus Game.2 is selected when the variant byte at `0x81D532` equals 2.
The routine is shared by Game.0/1/2; it is not a special Game.2 decoder.

The sequential reads establish these exact offsets:

| File offset | Bytes | Runtime destination | Read call instruction |
|---|---:|---|---|
| `0x00000` | `0x20000` | `0x659B0C` item instances | `0x43A91F` |
| `0x20000` | `0x1900` | `0x67C08C` groups | `0x43A936` |
| `0x21900` | `0x7D00` | `0x77DEB4` world objects | `0x43A94D` |
| `0x29600` | `0x109A` | `0x7891B4` transitions | `0x43A964` |
| `0x2A69A` | `0x7D000` | `0x78A250` entities | `0x43A97B` |
| `0xA769A` | `0x5E24` | `0x8149F0` settlements | `0x43A992` |

`0x1900 = 200*32`; `0x7D000 = 2000*256`. The last read ends at
`0xAD4BE`, exactly Game.2's EOF. No decompression or per-entity constructor
is interposed in these reads.

`0x43EE9E` is a CRT-style descriptor read wrapper with a Win32 `ReadFile`
path. `0x43ECA7 -> 0x43ECC9` reaches `CreateFileA`; flag `0x200` selects
the binary-read path. The loader does not check each read's returned byte count,
so opening successfully is not proof that a truncated file is rejected.

After the reads, L scans other tables, calculates the protagonist group's
`+18` using `0x411078`, and lowers its `+1C` if necessary. The routine
subsequently loads QUEST_R.RES; successful group/entity reads alone do not
guarantee its final success return.

### Callers and mode selection

**ORIGINAL-CODE VERIFIED (D, S, M).**

```text
0x436214 main dispatch
  new game, case 8:
    0x437E38 -> 0x43A870       load Game variant
    restore selected hero identity/stats into loaded Entity
    0x437FB2 -> 0x43ADF0(0)    load initial location

  location change:
    0x436668 -> 0x435870       remove/compact flagged departing entities
    0x436670 -> 0x4357A0       persist runtime world-object state
    0x436687 -> 0x43ADF0(0)    initialize new location

  load save:
    -> 0x4246C0
       -> 0x43A870            load baseline variant/resources
       -> overwrite tables from save
       -> 0x43ADF0(1)         restore location without random respawn
```

The new-game path first preserves the creation-time Entity in a temporary
256-byte buffer (`0x437E21`) and then restores selected fields after the
bulk load. This is a protagonist-only operation, not a global NPC initialization
pass. Authored NPC stats and inventories come directly from Game.2.

## 2. Location assets and collision state

**ORIGINAL-CODE VERIFIED (M).**

`0x43ADF0..0x43BEB9` uses current location ID at `0x81D6C4` to construct
`KONUNG.<decimal ID>` (string `0x4507B4`, references `0x43AFBF/0x43AFC4`).

| Konung.N offset | Bytes | Runtime destination | Read call |
|---|---:|---|---|
| `0x00000` | `0x6400` | `0x617FD8` terrain | `0x43B036` |
| `0x06400` | `0xC800` | `0x4FAEB0` collision-source bytes | `0x43B141` |
| `0x12C00` | `0x3E80` | `0x807250` GRAPH placements | `0x43B158` |
| `0x16A80` | `0x8CA0` | `0x80BD50` props | `0x43B16F` |

The source collision bytes are expanded into the u16 table at `0x4EE6B0`,
in nested loops of 320 by 160:

```c
uint16_t expand(uint8_t b) {
    if (b == 0xFF)         return 0x0FFF;
    if ((b & 15) == 15)    return (b << 8) | 0x0FFF;
    return b << 8;
}
// collision[x*160+y], byte address 0x4EE6B0 + x*0x140 + y*2
```

This is the collision state consulted immediately by the NPC placement path.
Konung.N supplies map geometry/passability and decorative placements; Game.2
supplies the NPC/group records activated for that location.

## 3. Group fields established by consumers

**ORIGINAL-CODE VERIFIED unless explicitly qualified (M, C, T, U, L).**

| Offset | Storage / interpretation on these paths |
|---|---|
| `+00` | First entity index; stored 16-bit, but traversal/constructor load it with `MOVSX` |
| `+02` | Signed 32-bit replenishment accumulator; decremented when actual population is below target |
| `+06` | Signed 16-bit target-growth countdown |
| `+08,+0C,+10,+14` | Rectangle A: x1,x2,y1,y2, unsigned 16-bit loads |
| `+0A,+0E,+12,+16` | Other interleaved rectangle fields; semantics unresolved here |
| `+18` | Target count for replenishment; party-size limit in the protagonist group |
| `+19` | Capacity ceiling, independently supported by reserved-slot layout |
| `+1B` | Location ID |
| `+1C` | Actual contiguous count; controls traversal, append slot, and averaging |
| `+1D` | Cleared in mode 0 for groups with `+1E & 0x40`; broader semantics unresolved |
| `+1E` | Class/flags: zero disables location group pass; bits `0x40/0x80` alter initialization |
| `+1F` | Not resolved in this investigation |

Important assembly detail: expressions such as
`*(int *)(g+0x19) >> 24` extract **signed byte `g[0x1C]`**. They do not mean
the field at `+19` is the traversal count. Similarly, a shifted dword at `+18`
extracts location byte `+1B`; one at `+1B` extracts class/flags byte `+1E`.

The relevant comparisons use signed branches. Storage declarations as u8/u16
do not fully describe how the original C/compiler treated those values.
The shipped records stay within the ordinary positive count/index ranges.

## 4. Existing NPC activation

**ORIGINAL-CODE VERIFIED (M); pseudocode omits unrelated asset management.**

```c
for (int gid = 1; gid < 200; ++gid) {
    Group *g = (Group *)(0x67C08C + gid*0x20);
    if (g->byte1E == 0 || (int8_t)g->location != current_location)
        continue;

    if (mode == 0 && (g->byte1E & 0x40))
        g->byte1D = 0;

    Entity *e = entity_base + (int16_t)g->first * 0x100;
    for (int j = 0; j < (int8_t)g->actual_1C; ++j, e += 0x100) {
        if (mode == 0 &&
            !(g->byte1E & 0x80) &&
            (int8_t)g->byte1E < 0x51) {
            if (!place_in_group_rectangle(e)) {
                g->actual_1C = j;
                break;
            }
        }

        if (!(e->byte04 & 0x80) && e->byte00 != 9 && e->byte00 != 3)
            collision[e->x*160 + e->y] |= entity_index(e) + 1;

        register_special_graphics_if_needed(e);
        // This also sets e[2]=0x40 in mode 0 for the 0x40 graphics family.
        if (mode == 0) reset_action_and_build_graphics(e, e->byte00); // 41435C
        else           rebuild_saved_graphics(e);                  // 414078
    }
}
```

Exact anchors:

- `0x43B32A`: `Group+1E != 0`.
- `0x43B333..0x43B33F`: location extraction/comparison.
- `0x43B35F`: signed first index; `0x43B381`: entity stride `0x100`.
- `0x43B38B..0x43B394`: actual count `+1C`.
- `0x43B39A..0x43B3B7`: mode/flags/class placement predicate.
- `0x43B3BF`: placement call; `0x43B3D1`: shorten `+1C` on failure.
- `0x43B43B..0x43B45F`: store one-based entity occupancy.
- `0x43B543 / 0x43B551`: reset versus saved-pointer rebuild.

There is no test for zero Entity coordinates in this block. For example, an
eligible record with nonzero persisted coordinates is still re-randomized in
mode 0. A record outside this predicate retains its coordinates even if zero.

Group 0 is excluded from this general NPC pass. The party is handled separately
by `0x4118DC` in mode 0 and by the save loader in mode 1.

## 5. Exact random placement

**ORIGINAL-CODE VERIFIED (P).** Entry `0x411DE4`, inclusive end `0x411FCE`.
Callers observed: constructor `0x420A8C` and location setup `0x43ADF0`.
Call instructions: `0x420F02`, `0x43B3BF`, `0x43B61B`.

```c
bool place_in_group_rectangle(Entity *e) {
    Group *g = group_base + (uint8_t)e->byte05 * 0x20;
    int hx = ((int)u16(g+0x0C) - (int)u16(g+0x08)) / 2;
    int hy = ((int)u16(g+0x14) - (int)u16(g+0x10)) / 2;
    int cx = (u16(g+0x08) + u16(g+0x0C) + 1) / 2;
    int cy = (u16(g+0x10) + u16(g+0x14) + 1) / 2;

    for (int attempt = 0; attempt < 100; ++attempt) {
        int dx = hx < 2 ? 0 : rng() % hx;
        int sx = rng() % 2 == 0 ? -1 : 1;  // consumed even when dx == 0
        u16(e+8) = cx + sx*dx;
        int dy = hy < 2 ? 0 : rng() % hy;
        int sy = rng() % 2 == 0 ? -1 : 1;  // consumed even when dy == 0
        u16(e+10) = cy + sy*dy;

        if ((collision[u16(e+8)*160 + u16(e+10)] & 0x0FFF) == 0)
            return true;
    }
    return false;
}
```

Division of signed differences truncates toward zero. Candidates are written
as 16-bit coordinates before testing. The rectangle is sampled around its
rounded-up center with magnitude modulo half-width/height; this is not a
uniform inclusive rectangle sampler.

The occupancy test is at `0x411F8F`; the 100-attempt bound is at `0x411EAD`.
There is **no x/y bounds comparison, clamp, coordinate conversion, or center
fallback anywhere in this function**. Valid authored rectangles are an implicit
precondition, not a verified bounds guard. Failure leaves the last attempted
coordinates written into the Entity and does not reserve its cell.

The 320x160 array shape is independently verified in M. That must not be
misreported as an explicit guard in P. The separate party-arrival routine
`0x4118DC` does have explicit coordinate checks, including interior bounds
`0 < x < 319`, `0 < y < 159`; those checks cannot be transferred to P.

### RNG

**ORIGINAL-CODE VERIFIED (RNG).**

```c
uint32_t rng(void) {                 // 0x43EA46
    uint32_t *s = rng_state();       // 0x43EA3C, runtime-context pointer + 0x0C
    if (!s) return 0;
    *s = *s * 0x41C64E6D + 0x3039;  // modulo 2^32
    return (*s >> 16) & 0x7FFF;
}
```

`0x43EA69` writes the seed. `0x43BFE4` obtains a value through
`0x43F7A2` and passes it to the seed setter. The exact source/time semantics
of that value and the complete interleaving of all RNG consumers were not
established. The recurrence itself is established; exact full-game spawn
reproduction still requires seed and call-order reconstruction.

## 6. Population growth and materialization

### Periodic bookkeeping

**ORIGINAL-CODE VERIFIED (T, TA, U).**

The main loop invokes `0x41D048`. Every 16 ticks it calls `0x41BD3C`
for groups with nonzero `+1E` and bit `0x40`. This includes groups outside
the current location; a separate current-location check gates other AI work.

```c
void update_population(Group *g) {        // 0x41BD3C, relevant fragment
    int k = (int8_t)g->byte1E - 0x41;
    if (k < 0x10) {                     // exact signed test; no added lower bound
        if ((int8_t)g->target_18 < (int8_t)g->capacity_19) {
            --i16(g+0x06);              // 0x41BD6E
            if (i16(g+0x06) < 0) {
                ++g->target_18;         // 0x41BD82
                i16(g+0x06) += i16(0x45E31E + k*4);
            }
        }
        if ((int8_t)g->actual_1C < (int8_t)g->target_18)
            --i32(g+0x02);              // 0x41BDAA
    }
    // Other action/AI processing follows; this fragment constructs no Entity.
}
```

Therefore `+06` governs target growth, while `+02` accumulates the deficit
used on location entry. The constructor's only direct caller observed by
Ghidra is `0x43ADF0`; this is a bounded static-reference finding, not proof
that no computed invocation exists.

### Location-entry replenishment

**ORIGINAL-CODE VERIFIED (M, C).** Runs only for mode 0, after existing NPCs.

```c
bool skip_first = true;
for (int gid = 1; gid < 200; ++gid) {
    Group *g = &groups[gid];
    if (!(g->byte1E & 0x40) ||
        (int8_t)g->location != current_location ||
        (int8_t)g->byte1E - 0x41 >= 0x10)
        continue;

    if (skip_first) { skip_first = false; continue; }

    while (i32(g+2) < 0 && (int8_t)g->actual_1C < (int8_t)g->target_18) {
        Entity *e = construct_population_member(g);  // 0x43B602
        if (!e) break;
        if (!place_in_group_rectangle(e)) break;    // 0x43B61B, second placement
        i32(g+2) += i16(0x45E216 + (int8_t)g->byte1E*4);
        reserve_collision_cell(e);
        register_special_graphics_if_needed(e);
        recompute_item_effects(e);                  // 0x41C69C at 43B753
        reset_action_and_build_graphics(e, e->byte00);
    }
    if (i32(g+2) < 0) i32(g+2) = 0;                // 0x43B77F
}
place_party();                                      // 0x43B78B -> 4118DC
```

The **first matching group is skipped**, even if it has a deficit. Its purpose
is unresolved. The class predicate is reproduced exactly as a signed
subtraction/comparison, not replaced by an invented inclusive range.

The second placement call is real. C has already placed the Entity and incremented
`+1C` before returning success. If the second placement fails, this caller
breaks without rolling the count back, reserving the new cell, or completing the
new-member graphics initialization. This is an observable static failure path,
not evidence that it occurs in normal gameplay.

### Constructor 0x420A8C..0x420F2C

**ORIGINAL-CODE VERIFIED (C).**

The append slot is `first + actual_1C`, not `first + target_18` or
`first + capacity_19`. The routine starts from the first member, averages
selected fields across actual members when count >1, and chooses one existing
member for appearance. It does not copy an entire Entity or clear all 256 bytes.

```c
Entity *construct_population_member(Group *g) {
    int n = (int8_t)g->actual_1C;
    Entity *members = &entities[(int16_t)g->first];
    // Seed aggregates from members[0], even if n is zero.
    aggregate = selected_fields(members[0]);
    if (n > 1) {
        for (int j=1; j<n; ++j) aggregate += selected_fields(members[j]);
        aggregate /= n;                       // signed integer means
    }
    int appearance = n < 2 ? 0 : rng()%n;
    Entity *e = &members[n];

    e->byte04 = g->byte1E;
    zero(e+0x66, 12); zero(e+0x72, 64); zero(e+0xB2, 10);
    e->byte00 = (e->byte04==0x4C || e->byte04==0x46 ||
                 e->byte04==0x55) ? 4 : 0;
    initialize_selected_fields(e, aggregate, members[appearance], g);
    if (!place_in_group_rectangle(e)) return NULL;
    ++g->actual_1C;                           // 0x420F11
    return e;
}
```

Selected initialization:

| Destination | Value |
|---|---|
| `+54,+56` | Mean of existing `+54` |
| `+58,+5A` | Mean of existing `+58` |
| `+5E,+60` | Mean of existing `+5E` |
| `+62,+64` | Mean of existing `+62` |
| `+33,+34,+4E,+52,+11` | Means of corresponding existing fields |
| `+5C` | `0x640` (1600 internal HP units) |
| `+01` | `rng()%8` |
| `+D6,+D8` | Visual byte and palette dword from the same chosen existing member |
| `+05` | `(group_pointer - 0x67C08C)/32` |
| `+02,+03` | `0x40,0x04` |
| `+D4,+D5,+D0` | `0,0,0xFF` |
| `+08,+0A` | Initially zero, then written by placement |
| `+10,+13,+50` | `0,0xFF,0` |
| `+12` | `rng()%5+1` only for class `0x4C`; otherwise not assigned here |
| `+66..+71,+72..+B1,+B2..+BB` | Cleared as three separate spans |

The stat names in the project are useful labels, but the exact offsets above
are the authoritative result. There is no capacity check inside C, no
whole-record zeroing, and no rollback of partially initialized fields on a
placement failure. The caller and valid group layout supply its preconditions.

## 7. From Entity state to initialized graphics

**ORIGINAL-CODE VERIFIED (I, R, F).**

Mode 0 calls `0x41435C(e, action)`. It:

1. Calls `0x438A78(&e[+E8], &e[+F0], u16(e+08), u16(e+0A))`.
2. Selects graphics family using `Entity+04 & 0x40`, not action/kind 6.
3. Resolves resource tables with visual byte `+D6`: block stride `0x4D28`
   for the clear-bit family, `0x3228` for the set-bit family.
4. Stores the action at `+00`, resets animation frame `+10` to zero,
   rebuilds frame/offset pointers at `+DC/+E0` and extent data at `+F4`.
5. Updates derived anchors `+E4/+EC` using `+E8/+F0`, resource offsets,
   and globals `0x81D738/0x81D754`.

For movement actions 1/7 it can also inspect a next cell and update Entity
coordinates and occupancy. Consequently the placement sample need not be the
final position after all action initialization.

The arithmetic of `0x438A78` is:

```c
*out0 = y*58 + (2-(x&1))*29;
*out1 = x*16 + 16;
```

The call from I establishes its use to derive these original runtime fields.
This report does not change or prescribe the remake's render projection; the
full terrain/render alignment dispute is outside this requested investigation.

Mode 1 calls `0x414078`, which resolves pointers using the saved action,
direction and existing animation frame. It does not call the coordinate
conversion or reset the frame. The saved-game path reads entire Entity records,
including their derived fields, before this rebuilding step.

Existing NPCs do not pass through the population constructor during the first
group loop. Recomputing item effects through `0x41C69C` is explicit in the
new-member replenishment path; it must not be generalized to every loaded NPC
without another caller trace.

## 8. Binary corroboration from Game.2

**BINARY-LAYOUT VERIFIED / DATA-CORRELATED, not runtime observations.**

- 192 groups have nonzero `+1E` (indices 0..191); 192..199 are empty.
- `+18 == +1C` in every authored group. Both total 1277; file values alone
  cannot distinguish their meanings.
- Capacity `+19` totals 1891. Consecutive populated groups satisfy
  `first[i] + capacity[i] == first[i+1]`; 614 reserved slots lie beyond
  current counts, with the last interval ending at slot 1891.
- Every traversed Entity's `+05` matches its owning group index.
- Of 1277 traversed entities, 979 have stored zero coordinates and 298 have
  nonzero coordinates. This correlation does not establish a zero sentinel.

Location 1 examples:

| Group | First | Target +18 | Capacity +19 | Actual +1C | +1E | Stored placement |
|---|---:|---:|---:|---:|---|---|
| 0 | 0 | 1 | 10 | 1 | `20` | Protagonist; separate path |
| 3 | 26 | 16 | 16 | 16 | `4F` | All zero; mode-0 placement eligible |
| 27 | 356 | 18 | 30 | 18 | `80` | Persisted; randomization disabled |
| 65 | 538 | 1 | 1 | 1 | `53` | Persisted; class threshold excludes randomization |
| 106 | 1019 | 6 | 8 | 6 | `4B` | All zero; mode-0 placement eligible |
| 107 | 1027 | 7 | 8 | 7 | `4B` | All zero; mode-0 placement eligible |

Location 1 has 79 counted entities including the protagonist: 51 NPCs with zero
stored coordinates and 27 NPCs with persisted coordinates. These are input
counts, not a prediction that all survive placement/initialization.

Entity 26 at file offset `0x2C09A` already has class `0x4F`, group 3,
level 10, dexterity field `+54=40`, HP `+5C=1600`, strength `+5E=60`,
endurance `+62=50`, visual `+D6=12`, palette `+D8=0x12800`.
Its zero coordinates do not make its authored stats an uninitialized template.

Group 10 (class `0x44`, location 37, entity 116) has nonzero coordinates
despite satisfying the mode-0 randomization class predicate. This is a concrete
counterexample to equating eligible classes with zero-coordinate records.

Group 82, location 27, has rectangle A `x=242..268, y=52..64`, reinforcing
that the first axis is not limited to 159. P writes those rectangle coordinates
directly to Entity fields and indexes the collision table with them.

## 9. Remaining questions and limits

1. Why does the location replenishment pass skip its first eligible group?
2. Why is placement repeated after C succeeds, and is the count-without-cell
   failure path reachable with normal authored data?
3. What are the exact design meanings of the two countdown tables
   `0x45E31E` and `0x45E216`, including their intended time units?
4. What do the second rectangle, `+1D`, `+1F`, and all class values mean
   beyond the verified tests and assignments?
5. What seed value and full RNG consumption sequence produce an exact original
   population layout? The LCG recurrence and unconditional sign draws are known.
6. Which untouched append-slot fields are intentional persistent/template data,
   and which depend on previously initialized memory?
7. Complete graphics completion, rendering, and all AI branches were not
   exhaustively reconstructed. No original game process was executed.
8. Reference lists are Ghidra's analyzed static references; unresolved computed
   calls can remain absent. A single direct constructor caller is not a universal
   proof of absence elsewhere.

Relevant findings that supersede hypotheses in older local notes for this
investigation: no bounds guard in P; no zero-coordinate placement predicate;
`+18` is distinct from the actual count; exact RNG recurrence is available.
The explicit user request forbids source/game edits, so those notes and remake
code were left unchanged.

## 10. Required CHECK work log

- **CHECK 1 — domain:** Game offsets identify raw file tables; Group and Entity
  positions in the traced spawn path share the collision-cell domain.
  Derived Entity anchors are separate runtime fields.
- **CHECK 2 — consumers:** L loads tables; M selects location groups; P consumes
  rectangle A; C appends members; T/U update population targets/deficits;
  I/R initialize or restore graphics.
- **CHECK 3 — conversions:** P writes group-sampled coordinates directly to
  Entity and collision addressing. Collision source bytes expand to u16 in M.
  I separately calls F for derived runtime anchors.
- **CHECK 4 — remake status:** No implementation change or broad remake audit
  was performed. Existing notes conflict with the observed placement guard and
  selection behavior; the above original-code trace is the evidence for a future
  narrowly scoped review, not authorization to alter rendering.
- **CHECK 5 — invented behavior:** None added. Pseudocode explicitly preserves
  signed comparisons, unconditional RNG sign calls, no bounds guard/fallback,
  the skip-first rule, partial constructor writes, and the second placement.

Requested investigation checklist: file loader **answered**; group traversal and
load modes **answered**; existing NPC initialization **answered**; random placement
**answered**; population target/timer/constructor path **answered with listed
semantic unknowns**; full runtime reproduction and remaining AI/design semantics
**unresolved/outside static coverage**.

Only new report/evidence artifacts were written. Original game files, existing
documentation, and remake source code were not modified.

