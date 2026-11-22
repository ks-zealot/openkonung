# Function parity — `konung.res` vs `web/src`

Status: **Wave A (AI) report, 2026-11-18.** Report-first: Wave A landed as a
read-only audit. **F-TR-01 and F-AI-01…F-AI-12 and F-AI-14 were approved and
fixed on 2026-11-18**; F-AI-13 is a recorded deviation, and no audited AI row is
open. **Wave B (trading) was audited on the same day**: 6 parity, 1 deviation and
**F-TR-02 fixed** (`0x41079C`, the XP/auto-spend engine).

Target: `konung.res`, PE32/i386, image base `0x400000`,
sha256 `d698c4f231489a372cc25212ab2819720a36d7ce5307a045db684135cf679810`.
Provider: REA / Ghidra 12.1.4 (default auto-analysis).

---

## 0. Summary

| system | functions | audited | parity | deviation | platform | gaps open | fixed |
|---|---:|---:|---:|---:|---:|---:|---:|
| AI | 44 | 44 | 27 | 1 | 2 | 0 | 14 |
| TRADE | 13 | 13 | 11 | 0 | 0 | 0 | 2 |
| COMBAT | 20 | 20 | 10 | 0 | 1 | 0 | 9 |
| AUDIO | 12 | 12 | 2 | 0 | 10 | 0 | 0 |
| SETTLEMENT | 17 | 17 | 12 | 0 | 0 | 0 | 5 |
| QUEST | 66 | 66 | 53 | 0 | 0 | 0 | 13 |
| MOVEMENT | 14 | 14 | 13 | 0 | 1 | 0 | 0 |
| RENDER | 30 | 30 | 22 | 0 | 8 | 0 | 0 |
| INPUT | 9 | 9 | 9 | 0 | 0 | 0 | 0 |
| LOADER | 10 | 10 | 4 | 0 | 6 | 0 | 0 |
| MISC (unclassified) | 56 | 56 | 45 | 0 | 6 | 0 | 5 |
| **total** | **291** | **291** | **208** | **1** | **34** | **0** | **48** |

Wave A found **15 gaps** in 19 audited functions (13 in AI, 2 in trade),
including three that directly explain a companion behaving differently from the
original, two that make every archer in the game shoot at a quarter of the
original range, and a 4× error in the trade-skill price coefficient.
**F-TR-01 and F-AI-01…F-AI-12 and F-AI-14 are fixed** (2026-11-18). F-AI-13 is
a recorded `PARITY-DEVIATION` (staged movement/citizen/follower passes); no
audited AI row is left open.

**Wave B (trading)** adds 13 audited rows: 9 parity, 2 documented deviations
(`0x41EAD8` held-item sentinel, `0x417010` ground-container window) and
**F-TR-02** (`0x41079C` XP/auto-spend) is **fixed**: kills now pay `power/4`
experience plus `power` mastery, and AUTO spends it. **Wave B is complete.**

The headline corrections:

1. `FUN_0041F3C8` (companion follow) is **not** an "8-neighbour ring": it picks
   a slot from a 12-entry, direction-and-parity author table using `rand()%12`,
   retries four times, and parks the companion **1–7 cells** from the leader
   (§F-AI-01, **fixed**).
2. `FUN_0041130C` (line-of-fire) advances **~4 px per sample**, not 1 px — the
   remake's archers probed at ¼ range (§F-AI-02, **fixed**).
3. The archer gate tests `Entity+0x68` = **`equipment[1]` (the bow)**, not
   `equipment[0]`; three remake sites tested the melee slot (§F-AI-03, **fixed**).
4. The original **throttles AI to three actions per frame** through
   `FUN_004116CC` (`_DAT_0081D5F0 < 3`) with a round-robin cursor
   (`_DAT_0081D568`). The remake runs every unit every tick (§F-AI-04, **fixed**).
5. `KONUNG_SOURCE_OF_TRUTH.md` §25 #4 ("exact original RNG unknown") is stale:
   `rand()` is the MSVC LCG `state = state*0x41C64E6D + 0x3039`,
   `(state>>16)&0x7FFF`, and it is already ported exactly. Exact seeded replay
   is therefore possible; the *number and order of draws* is now a parity
   requirement, and gap 1 breaks it.
6. Trade pricing used a skill coefficient of 0.008; the binary stores **0.002**,
   so every buy/sell price was wrong (§F-TR-01 — **fixed**).

---

**Status: all 291 rows audited (2026-11-20); deviation close-out complete.**
208 parity, 1 PARITY-DEVIATION (`0x421B50`, interleaved frame pass, wontfix),
34 N/A-PLATFORM, 48 fixed (GAP-*), 0 open.

## 1. How to read this

### 1.1 The ledger

`evidence/function_parity.csv` has one row per game-range procedure (291 rows),
generated and validated by `web/tools/parity_index.py`:

```
va, csv_role, system, rust_symbols, verdict, fix_status, evidence_id, divergence, test
```

* `rust_symbols` — every `web/src` file that cites the VA (refreshed on every
  run of the tool, so it cannot rot).
* `verdict` — `PARITY`, `PARITY-DEVIATION`, `GAP-BRANCH`, `GAP-VALUE`,
  `GAP-ORDER`, `GAP-STATE`, `GAP-MISSING`, `N/A-PLATFORM`; empty = not audited.
* `fix_status` — `open`, `approved`, `fixed`, `wontfix`, `n/a`.

Commands:

```powershell
cd web
python tools\parity_index.py           # refresh the ledger / citation index
python tools\parity_index.py --check   # fail on holes, bad verdicts, dead paths
python tools\parity_index.py --system AI
python tools\parity_set.py findings.json   # write audited fields
```

`--check` currently reports **272 unaudited** rows and no structural problems.
That number is the honest coverage statement: the port is annotated, not proven.

### 1.2 The inventory's `role` strings are not evidence

`evidence/konung_res_functions.csv` is a name inventory. Eight of its labels are
wrong in ways that would misdirect a port; all were re-derived from the binary
during this pass:

| VA | CSV role | verified meaning | evidence |
|---|---|---|---|
| `0x429570` | random seed/time init | MCI music open (`mciSendCommandA(…,0x803…)`) | `procedure_pseudo_code` this pass |
| `0x41AE00` | damage scaling by strength | **sell price** (`total*0.5*(1+diff*0.002)`) | §6 |
| `0x41B5A8` | damage scaling by level/strength | **buy price** (`total*(1-diff*0.002)`) | §6 |
| `0x41D048` | death/action transition | settlement pass (the 16-frame pass) | AGENTS §settlement, 23 citations |
| `0x4175FC` | crafting/assembly slot processor | merchant restock, role 2 | `SETTLEMENT_SYSTEM.md` |
| `0x4179C8` | generate loot/treasure | merchant restock, role 4 | `SETTLEMENT_SYSTEM.md` |
| `0x4203F4` | party inventory transfer | trade confirm/cancel validation | `PARITY_GAPS_PLAN.md` §0 |
| `0x41B658` | full attack damage roll | entity combat-power metric (settlement ratio) | §7.3 |

Consequence: the ledger stores `csv_role` verbatim for traceability and treats
only the decompiled body as evidence.

### 1.3 What "parity" means here

There is no runnable original oracle (1999 DirectDraw/DirectSound binary,
time-seeded RNG). Parity therefore means, per function:

1. same fields read/written, same constants and tables;
2. same branch order and same returned codes;
3. same number and order of `rand()` draws;
4. same tick/ordering relationship to its callers.

Items 1–3 are decidable statically. Item 4 is where most of the AI gaps live.

---

## 2. Wave A scope

Audited (16 AI + 2 trade + 1 platform):

`0x421B50`, `0x41ECC8`, `0x41E2D4`, `0x41EB48`, `0x41EFD8`, `0x41F3C8`,
`0x4116CC`, `0x410E84`, `0x41130C`, `0x4101E4`, `0x41FB64`, `0x420190`,
`0x420098`, `0x41FF68`, `0x412C30`, `0x412E00`; plus `0x41AE00`, `0x41B5A8`,
`0x429570`.

Scheduled, not audited (28 AI rows): the action machine `0x4123BC`, the citizen
chain `0x415FC8`/`0x4168A4`/`0x415C40`/`0x413A84`, the arrival/reset helpers
`0x413DE4`/`0x414A34`/`0x410450`, the projectiles `0x41BF44`/`0x420F30`, the
damage chain `0x41AB7C…0x41C3CC`/`0x41C69C`, and the movement helpers
`0x4142D4`/`0x41435C`/`0x414ABC`/`0x43DA1F`/`0x43D922`/`0x410FFC`/`0x41165C`/
`0x438090`/`0x438640`/`0x43881C`/`0x43897C`/`0x438ED8`.

---

## 3. Findings — `F-AI-01 … F-AI-13`

Severity: **S1** changes what the player sees in normal play; **S2** changes
outcomes in combat/settlement; **S3** ordering/edge-case only.

### F-AI-01 (S1) — companion follow formation is a random authored table
**VA** `0x41F3C8` · `GAP-VALUE` · `ev_9285ede2…`
**Status: fixed 2026-11-18** (`constants/motion.rs`, `actors/motion/formation.rs`).
**Original.** `FUN_0041EFD8` → `FUN_0041F3C8(group, member)`:

```
if group.class & 0x40 != 0            -> return 0
leader = entities[group.first_entity]
if FUN_0043881C(leader.cell, member.cell) < 10 -> return 0
dir    = FUN_0043897C(member.cell, leader.cell)         // 0..7
parity = leader.x & 1
for attempt in 0..4:
    start = rand() % 12                                 // one draw per attempt
    for i in 0..12:                                     // start..start+11 (wrap)
        cell = leader.cell + TABLE[parity][dir][start+i]
        if in bounds (0<x<0x13F, 0<y<0x9F) and collision[cell] & 0xFFF == 0:
            member.dest = cell; member.order |= 0x48; member.+0x14 = 0xFF
            return 1
        cell = leader.cell + TABLE[parity][dir-1][start+i]   // alternate direction
        (same test)
return 0
```

`TABLE` is `2 × 8 × 12` signed `(dx,dy)` pairs at `0x45FF19` (payload read at
`+3/+4` of each 2-byte slot — the decompiler's `>> 0x18` on the words at
`base+off` and `base+off+1`). Decoded, direction 4/parity 0 is:

```
(1,1) (1,2) (0,2) (3,2) (2,2) (1,3) (3,3) (2,3) (5,3) (4,3) (3,4) (1,4)
```

i.e. a fan of slots **1–7 cells** from the leader, mirrored per x-parity and
rotated per direction. So companions hold a loose formation well away from the
leader and re-form on a random slot.

**Remake.** `actors/motion/formation.rs::follow_goal` returns the first free cell
of a fixed 8-neighbour ring around the leader, then the leader's own cell; no
RNG is drawn. Formation collapses to a tight ring and the global `rand()`
sequence diverges from the original from the first follow tick onwards.

**Impact.** Visible: companion spacing/shape is wrong. Behavioural: every later
`rand()` consumer (damage, dodge, loot quality, wander, settlement money) is
shifted. This is the single largest source of "the AI feels different".

**Fix.** Port the table (constant array `FOLLOW_SLOTS[2][8][12]`), the
`rand()%12` start, the 12-entry scan, the `dir-1` alternate and the 4 attempts.
Gate on `follow_distance >= 10` (already correct).

**Applied.** `constants/motion.rs` carries the 192 decoded pairs
(`FOLLOW_SLOTS`, payload at `0x45FF1C`), and `follow_goal` now takes the
member's cell and `&mut OriginalRng`, draws the start slot per attempt, scans all
twelve slots trying `dir` then `dir-1`, and honours the original bounds
(`0 < x < 0x13F`, `0 < y < 0x9F`) and the `collision & 0xFFF == 0` free test.
`update_followers` gained the RNG parameter and `app/party.rs::update_party`
passes `combat_rng`, so the follower draws from the same sequence as combat and
the state is saved with the snapshot.

**Verification.** `actors/tests/motion.rs`:
`follow_slots_match_the_authored_table` (shape, spot entries, all offsets within
1–7 cells), `follow_goal_draws_a_random_start_slot` (exactly one draw, goal is
`leader + FOLLOW_SLOTS[parity][dir][start]`), and
`follow_goal_retries_four_times_and_consumes_four_draws` (fully blocked grid →
`None` after exactly four draws).

**Test.** Table bytes → `FOLLOW_SLOTS` digest; deterministic `OriginalRng` seed
producing a known start index; a member at distance 10 lands on the expected
slot; all 12 blocked → 4 draws consumed and no move.

---

### F-AI-02 (S1) — line-of-fire steps 4 px per sample, not 1
**VA** `0x41130C` · `GAP-VALUE` · `ev_5853563a…`
**Status: fixed 2026-11-18** (`web/src/combat/los.rs`, `los/tests.rs`).
**Original** (`0x4114a5`…`0x4114e8`):

```
dxn = |aim.x - src.x| * C + D        // C = f64 @0x45000C = 0.25
dyn = |aim.y - src.y| * C + D        // D = f64 @0x450014 = 0.001
den = max(dxn, dyn)
step = (Δx/den, Δy/den)              // dominant axis advances 1/0.25 = 4 px
repeat 8 * shots: { src += step; cell = to_cell(round(src.x), round(src.y)+0x1E); ... }
```

**Remake.** `combat/los.rs` uses `adx = |dx| + 0.001`, `ady = |dy| + 0.001`, so
the dominant axis advances 1 px per sample. The module comment states this
explicitly ("unit DDA", "1-pixel-step"), which is the source of the error.

**Impact.** `shots` is the weapon's authored sample count (1 for melee subtypes,
18/25 for ranged), so the original probes 4× further per shot budget. Remake
archers cannot see, and therefore cannot acquire or keep, targets beyond roughly
a quarter of the original distance.

**Applied.** `los.rs` now has `STEP_SCALE = 0.25` and a pure `step_for(dx, dy)`
helper that divides both deltas by `max(|dx|,|dy|) * 0.25 + 0.001`, so the
dominant axis advances 4 px per sample. The module/`probe` docs no longer claim
a "1-pixel-step DDA".

**Verification.** `los/tests.rs::the_dda_advances_four_pixels_per_sample` pins
the step (`±400 px → ±4 px`), the scaled dominant-axis choice (4:1 delta → 1/4)
and the zero-length case; `a_long_open_line_is_reachable_within_the_sample_budget`
pins reachability end-to-end (20 cells ≈ 1160 px is clear with 40 units, not
with 10).

---

### F-AI-03 (S1) — the archer gate reads the bow slot, not the melee slot
**VA** `0x410E84`, also read by `0x41130C` and `0x41ECC8` · `GAP-VALUE` ·
`ev_45ea577c…`
**Status: fixed 2026-11-18.**
**Original.** `if (raw_0x12 == 1 && u16@+0x68 != 0 && u16@+0xCC != 0)`.
`+0x68` is `equipment[1]` (the bow); `+0xCC` is the ammo id. `0x41130C` then
derives `shots` from **that** item's subtype:
`u16[0x459B84 + subtype(equipment[1])*0x40]`.

**Remake.** `app/combat/ai.rs::ranged_probe_range` and
`app/combat/acquire.rs::probe_range` both test `entity.equipment[0] != 0` and
`item_class_subtype(weapon)` of slot 0 (`los.rs` docs repeat "active melee
weapon's subtype").

**Impact.** A companion/NPC holding a bow but no slot-0 weapon never enters the
armed path; one holding a slot-0 weapon and a bow uses the wrong subtype's
sample count. Every archer in the shipped data is affected.

**Fix.** Use `equipment[1]` at all three sites; keep `raw_0x12 == 1` and
`special_equipment != 0`.

**Test.** Entity with `equipment = [0, bow, …]`, ammo set, `raw_0x12 = 1` →
`Some(shots)`; same entity with the bow moved to slot 0 → `None`.

**Applied.** The predicate lives in one place now,
`app::combat::armed_bow(slot, equipment, ammo) -> Option<bow_id>`, used by
`ai.rs::ranged_probe_range` and `acquire.rs::probe_range`; both read the subtype
of the returned **bow** id. `app/tests/combat_ai.rs::armed_gate_reads_the_bow_slot`
pins the field (slot 0 only → `None`, slot 1 → `Some`, wrong active slot → `None`,
no ammo → `None`).

One behavioural correction fell out of the fix: the old code turned a failed
line-of-fire pick into "no target at all". `FUN_00412E00` falls through to
`FUN_00420098` (plain nearest) when `FUN_00410E84` returns 0, so
`ai.rs::update_world_ai` now keeps the LOS-clear subset when non-empty and
otherwise uses the unfiltered candidate list.

---

### F-AI-04 (S1) — AI runs three actions per frame, round-robin
**VA** `0x4116CC` · `GAP-MISSING` · `ev_575ef481…`
**Status: fixed 2026-11-18** (`app/combat/schedule.rs`, wired in `tick.rs`,
`ai.rs`, `unit_ticks.rs`).
**Original.**

```
FUN_004116CC(entity) = _DAT_0081D5F0 < 3 && _DAT_0081D568 < entity
```

`_DAT_0081D5F0` is an action budget: reset to 0 at `FUN_00421B50` entry,
incremented by the equipment refresh (`0x4105AC`), the armed pick (`0x410F0D`),
each line-of-fire probe (`0x4113EC`) and the citizen AI (`0x416D62`).
`_DAT_0081D568` is a cursor set to the last entity that passed the gate; at
dispatcher exit, if it did not advance, it resets to 0. Entities are visited in
group/entity order, so the gate serves **the next up-to-three eligible units per
frame, continuing where the previous frame stopped**.

The gate is used twice per entity: on the idle branch
(`+0x02 & 0x40` + `FUN_004116CC` before `FUN_0041EFD8`) and before the walk
branch's `FUN_0041ECC8`.

**Remake before the fix.** No equivalent: `combat_tick` ran every unit every
60 ms tick, and `FUN_004105A0` was applied unconditionally.

**Impact.** AI cadence, and therefore target acquisition order, differed whenever
more than three eligible units exist, and the number of RNG draws per frame did
not track the original (an archer probe consumes budget and can starve later
units).

**Applied.** `app/combat/schedule.rs::AiSchedule` models the pair exactly:
`allows(order)` = `actions < 3 && cursor < order`, `serve(order)` writes the
cursor, `charge()` increments the budget, `begin_frame()` clears the budget and
saves the cursor, `end_frame()` zeroes the cursor when the frame served nobody.
Order keys are one-based (`entity.index + 1`) because the original compares
entity **addresses** (never zero) while zero is the "nothing served" sentinel.

Wiring:

* `combat_tick` now opens/closes the AI frame and runs **group 0 first**
  (leader, then companions) before the object/citizen groups, matching the
  original group-index traversal.
* `update_player_combat` gates acquisition + retaliation on the leader's turn
  (the attack cycle still advances, like the animation inside `FUN_00421B50`).
* `update_party_combat` takes one gate decision per member up front and reuses it
  for both the re-acquire loop and the retaliation pre-pass.
* `update_world_ai` gates each group member in its existing group/member order,
  charges 1 for the armed pick and 1 per line-of-fire probe.
* `debug.rs` reports `ai_actions`/`ai_cursor` for the headless smokes.

The cursor's reset rule reproduces the original's stutter: a frame that serves a
unit leaves the cursor at its order, so units *below* that order are skipped on
the next frame, and the frame after that (where nobody is served) resets it.
Consequence: a lone unit re-decides every other frame — the original's behaviour,
because only the decision routine is gated while movement/animation run every
frame.

**Not wired (deliberate, separate findings):** the citizen AI charge
(`0x416D62` inside `FUN_004168A4`, which the remake drives from a different
timer), the equipment-refresh cadence (`FUN_004105A0` is called from
`FUN_00412E00`/`FUN_004168A4`, not per tick — F-AI-12), and follow-through
gating of the follower timer (F-AI-08). The merged per-entity dispatcher
(F-AI-13) is still approximated by "group 0 pass, then world pass", which matches
the original's group order for the two populations that exist today.

**Verification.** `schedule.rs` unit tests cover the budget gate, the
`begin_frame` cursor hand-over and the `end_frame` reset (including the
"served only entities below the cursor" case); `combat_ai.rs` asserts the armed
pick charges the budget, and the four re-acquire/retaliation tests now step whole
frames through the test-only `combat_tick_once`. Four of them needed more than
one frame because of the cursor stutter — that is the behaviour under test.

---
### F-AI-05 (S1) — retaliation gate and state writes
**VA** `0x41EB48` · `GAP-STATE` · `ev_7267e36a…`
**Status: fixed 2026-11-18** (`unit_ticks.rs`, `ai.rs`).
**Original.**

```
if (+0x02 & 9) != 0 -> return 0                     // engage or move set: not idle
for dir in 0..8:
    cell = step(entity.cell, dir)
    t = collision[cell] & 0xFFF
    if t in {0, 0xFFF}                            -> continue
    if entities[t].group  == entity.group         -> continue
    if entities[t].+0x02 & 1 == 0                 -> continue   // attacker has engage order
    if entities[t].target != my one-based id      -> continue
    +0x06 = t; +0x0C/+0x0E = cell
    +0x02 &= 0xFB; +0x02 |= 2                     // target lock
    +0x13 = 0xFF                                  // clear direction queue
    +0x03 |= 4                                    // battle stance
    FUN_0041435C(entity, 5)                       // play melee row immediately
    if entity == leader: FUN_00428DB0(1,10)       // party marker refresh
    return t
```

**Remake.** `adjacent_world_attacker` scans the population for
`archetype & 0x40 != 0`, `combat_target == Some(me)`, Chebyshev-1 adjacency.
Missing: the `+0x02 & 9` gate, the **different-group** test, the attacker's
engage-order bit, the target-lock/queue/row-5 writes.

**Impact.** Mobs and citizens retaliate in cases the original ignores (any group
member, no engage order), and the retaliation does not start the original's
row-5 attack immediately.

**Applied.** `adjacent_world_attacker` now checks the **different group**
(group 0 = leader + party is excluded), the attacker's `engaged` flag
(`Entity+0x02 & 1`; `FUN_004142D4(...,1)` writes the whole order byte, so a
world unit that acquired a target now latches `engaged` in `update_world_ai` and
clears it when the target dies), and the target identity. The object-family
filter is gone. Callers add the `+0x02 & 9` gate: the leader must not be engaged
and the companion must hold no engage order.

The remaining writes already happen in the same tick's attack cycle (battle
stance, direction-queue clear via `path.clear()`, melee row 5 when adjacent), so
no duplicate row start is issued.

**Verification.** `app/tests/combat_ai.rs::retaliation_requires_the_attackers_engage_order`
(adjacent foreign actor without an order → no retaliation) plus the existing
positive retaliation test, which now runs through the gated dispatcher frames.

---

### F-AI-06 (S2) — recall in rectangle A is a random cell, not a clamp
**VA** `0x412E00` (activity `0x20`) · `GAP-BRANCH` · `ev_021d5a46…`
**Status: fixed 2026-11-18** (`ai.rs::return_group_to_rect` +
`recall_wander_goal`).
**Original.**

```
if (+0x02 & 1) != 0:
    g = FUN_0041FB64(group); pick target by archetype switch
    FUN_004142D4(entity, target.cell, 1)
else:
    // random point inside rectangle A, then a forward free-cell scan
    off = (dim-1)/4 < 2 ? 0 : rand() % ((dim-1)/4)     // two draws, one per axis
    x = x1 + off  /  x = x2 - off   (one axis adds, the other subtracts)
    scan up to (w*h) cells forward, wrapping at the rect edge
    FUN_004142D4(entity, x, y, 8)
```

**Remake.** `return_group_to_rect` clamps each stray member to the rectangle
(`cell.clamp`), walks straight there, and never chases a hostile while in
recall. No RNG.

**Impact.** Recall movement path and RNG consumption differ; an engaged unit
outside the rectangle does not chase in the remake.

**Applied.** `return_group_to_rect` is now the two-branch port:

* `engaged` → nearest party target (`FUN_0041FB64` for a world group resolves to
  the party), writes `combat_target` + marker and issues a mode-1 chase.
* otherwise → `recall_wander_goal`: `rand()%2` picks the sign, then one
  `rand() % ((dim-1)/4)` per axis when that span is at least two quarters, the
  point is clamped into `[x1+1, x2-1] × [y1+1, y2-1]`, then the forward
  free-cell scan walks at most `(w-1)*(h-1)` cells with the original wrap.
* already inside rectangle A → idle action only.

`EnemyRef` gained the one-based `marker` so the chase writes `target_marker`
exactly like the acquisition pass. Walking members still finish their current
cycle (the remake commits one cell per cycle, the original re-issues a route on
every recall pass).

**Verification.** `recall_member_wanders_to_a_random_cell_inside_the_rectangle`
(goal inside rect, not the clamped corner, RNG seeded) and
`engaged_stray_chases_the_party_in_recall_mode`.

---

### F-AI-07 (S2) — `0x41ECC8` ranged branch re-probes every cycle
**VA** `0x41ECC8` · `GAP-BRANCH` · `ev_9918bae4…`
**Status: fixed 2026-11-18** (`acquire.rs::armed_los`, three ranged blocks in
`unit_ticks.rs`).
**Original** (`+0x02 & 4` set):

```
if raw_0x12 != 1 || +0x68 == 0 || +0xCC == 0: +0x02 ^= 4; idle; return
if +0x06 == 0 && FUN_00410E84() == 0: idle; return
target entity: if invisible or kind 9/3 -> +0x06 = 0; ... idle
copy target cell to +0x0C/+0x0E
r = FUN_0041130C(target.cell, target id)
r > 0: FUN_0041435C(ent, 4); +0x02 &= 0xBF; return
r < 0: +0x02 ^= 4                          // toggle the ranged bit
+0x06 = 0; (if leader) +0x02 &= 0xFB; idle
```

**Remake.** `update_party_combat` plays row 4 and spawns an arrow whenever
`ranged_order` and a live target exist; the LOS probe is only used at
acquisition time.

**Impact.** Archers keep firing when the original stops (blocked line, target
became non-visible), and never toggle out of the ranged order.

**Applied.** `app/combat/acquire.rs::armed_los` wraps the gate + bow subtype +
`los::probe` into one call, and every ranged block (leader, companion, world)
runs it before starting a cycle:

* `Clear` → play row 4 / spawn as before.
* `Blocked` (`0`) → clear `combat_target`; a companion/world unit keeps the
  ranged order (retries next tick), the leader drops it (`+0x02 &= 0xFB`).
* `TooClose` (`-1`) → clear the target and the ranged order.
* not an armed archer (`None`) → clear the ranged order and keep the target
  (the gate branch: `+0x02 ^= 4`, idle).
* `CombatTarget::Prop` targets are exempt: a prop is a mouse-path target
  (blocked cell, no `Entity+0x06` id), not an actor-logic one, so the original
  never runs this probe for it.

`ranged_debug_report` now exposes `los=` (-2 = no probe, -1/0/1 as above) so the
headless smokes can see the decision.

**Harness fix.** `debug_ranged_setup` used to stage a live object-family target
4 cells away without freezing it; with the per-cycle probe the mob closed inside
the three-cell minimum before the 6-frame arrow spawn and the staged shot was
dropped — correctly. The helper now writes the staged cell to the canonical
`tables.entities` row, pins the target's walk frames and clears its group's class
bits so the smoke tests arrow flight rather than a chase.

**Verification.** `combat_ai.rs::armed_archer_app` builds a bow/ammo archer;
`ranged_cycle_drops_the_order_when_the_bow_gate_fails` (ammo removed → bit 4
cleared, target kept) and `ranged_cycle_clears_a_too_close_target` (distance 1 →
target and order cleared). `ranged_smoke` passes end-to-end again, and the
existing archer test moved from 2 to 4 cells because a 2-cell target is now
correctly rejected.

---

### F-AI-08 (S2) — follow is gated on the AI order bit, before the actor logic
**VA** `0x41EFD8` · `GAP-ORDER` · `ev_8d092b23…`
**Status: partially fixed 2026-11-18.** The predicates are ported; the follower
timer remains (F-AI-13).
**Original.** Called only from the idle branch of `FUN_00421B50`, only when
`(+0x02 & 0xF) == 0`:

```
if FUN_00410E84(entity) != 0     -> 1     // armed archer handled
if FUN_0041EB48(entity) != 0     -> 1     // retaliation handled
if (+0x02 & 0x20) == 0           -> 0     // AI bit off
return FUN_0041F3C8(group, entity)        // follow
```

**Remake.** `update_party` runs the follower pass on its own timer
(`party_step_t`), independently of the combat tick, with no armed/retaliation
pre-check and no `+0x02 & 0x20` gate.

**Impact.** Remake companions can start a follow move in the same frame they
retaliate or shoot; the original serialises armed → retaliate → follow and skips
follow entirely while any order bit is set.

**Applied.** `update_followers` now skips a member with a live `combat_target`
or a held `engaged` order, so the armed/retaliation outcomes and the order bit
suppress follow exactly like `FUN_0041EFD8`. `engaged` is also latched for world
actors on acquisition (F-AI-05), which is what makes the party-side predicate
meaningful.

**Still open.** The follower pass still runs on `party_step_t` rather than inside
the gated dispatcher turn, so within one frame a companion can retaliate and then
start a follow move; only the persistent order suppresses it. That is the
F-AI-13 merge.

**Verification.** `actors/tests/motion.rs::engaged_companions_do_not_follow`
(engaged member 20 cells away never routes).

**Test.** Unit with an engage order set → follow does not run; armed unit with a
target → follow does not run.

---

### F-AI-09 (S3) — `0x41E2D4` clears the ranged bit on re-acquire
**VA** `0x41E2D4` · `GAP-STATE` · `ev_c12d924f…`
**Status: fixed 2026-11-18** (`unit_ticks.rs`).
**Original.** Keep the current target while it is visible (no distance limit):
copy its cell to `+0x0C/+0x0E`, return 1. Otherwise `+0x06 = 0`, then
`FUN_0041FB64(own group)`; if none, `+0x02 &= 0xFE` and return 0; else
`FUN_00420190` then `FUN_00420098`; on success write `+0x06` and the cell and
**clear bit 4** (`+0x02 &= 0xFB`).

**Remake.** `acquire_next_hostile` always starts from the nearest hostile group
and writes `ranged_order = ranged` after a successful acquire.
`update_party_combat` also clears the engaged flag when nothing is found, but
sets the ranged bit from `ranged_ready` rather than clearing it.

**Impact.** A re-acquiring archer in the remake immediately re-enters the ranged
cycle; the original drops to melee approach for the first tick after acquiring.

**Applied.** Both re-acquire paths (leader and companion) now write
`ranged_order = false` / `player_attack_ranged = false` after a successful
acquire; the pick still uses `ranged` for the `FUN_00410E84` line-of-fire filter,
so the *choice* of target is unchanged.

**Verification.** `app/tests/combat_ai.rs::reacquire_clears_the_ranged_order`
(leader re-acquires the second ant after a kill with `player_attack_ranged` set
beforehand, and the flag comes back false).

---

### F-AI-10 (S3) — shared-target selection order
**VA** `0x41FF68` · `GAP-ORDER` · `ev_397eda8d…`
**Status: fixed 2026-11-18** (`ai.rs::update_world_ai` `shared`).
**Original.** Iterates the **caller's own group** in member order and returns the
target of the first member with `+0x02 & 1` and a non-zero target that is
visible/alive.
**Remake before the fix.** `shared_target` iterated the enemy candidate list and
returned the first engaged entry.
**Applied.** The group's shared target is now computed before the member loop:
the first member (in `pop.actors` order) with `engaged` and a live, visible
target. `target_is_visible` reuses the live party/enemy lists (`FUN_0041FF68`
checks the target entity's archetype/kind). `shared_target` is removed.
**Verification.** `shared_target_follows_member_order`: two already-engaged ants
target the companion and the leader; a third ant adopts the **first member's**
companion target, whereas the old enemy-order search would have returned the
leader (the enemy list is `[leader, companion]`).

### F-AI-11 (S3) — `0x41FB64` search rectangle depends on the caller
**VA** `0x41FB64` · `GAP-STATE` · `ev_68b3655a…`
**Status: fixed 2026-11-18** (`ai.rs::return_group_to_rect` object branch).
**Original.** For a party-class caller (`class & 0x20`) the box is
`first_entity.cell ± (r + r/2)` (30/13 at 800×600); for any other caller it is
the caller group's **rectangle A**. An object-group caller instead scans the
party group inside its own rectangle A and returns group 0.
**Remake before the fix.** `nearest_hostile_group` always used the leader-centred
box; recall chases ignored the rectangle entirely.
**Applied.** The recall chase is the object-group branch: candidates are the
party members **inside the caller's rectangle A**, nearest to the unit, and an
empty box means no hostile group (no chase). The party→world direction keeps the
leader-centred box unchanged.
**Reachability.** Recall (`0x20`) is set while no party member is inside the
rectangle, so this branch only fires in the 16-frame window after the party walks
in and before the relation pass flips activity to `1`; both tests model that
window explicitly.
**Verification.** `engaged_stray_chases_the_party_in_recall_mode` (party inside
the rect → chase) and `engaged_stray_outside_the_rect_finds_no_hostile_group`
(party outside → no target).

### F-AI-12 (S3) — `0x412E00` short-circuits three archetypes
**VA** `0x412E00` · `GAP-BRANCH` · `ev_021d5a46…`
**Status: fixed 2026-11-18** (`ai.rs::update_world_ai`).
**Original.** `if archetype in {0x51, 0x52, 0x53} return 1` (handled, no AI).
Also: an activity-1 citizen with no battle stance gets `FUN_004105A0` and is
**forced** into battle stance (`+0x03 |= 4`), then the archer check runs.
**Remake before the fix.** `update_world_ai` filtered only on `archetype & 0x80`
and death rows, and set `FLAG_BATTLE` on non-object archetypes *after* target
assignment.
**Applied.** The member loop now returns early for `0x51`/`0x52`/`0x53` (no
target, no animation change) and, for a citizen (`class & 0x40 == 0` with
`archetype & 0x40 == 0`) that is not in battle stance, forces `FLAG_BATTLE`,
sets the idle action and skips the target pick for that frame — the original's
`return 1` after `FUN_004105A0`. The equipment refresh itself already runs in
`refresh_npc_equipment` before the AI pass.
**Verification.** `prop_archetypes_never_acquire_targets` (0x52 gets no target)
and `woken_citizen_enters_battle_stance_before_acquiring` (stance set, no target
on the entry frame); `citizen_combat_target_is_not_reset_by_wander_pass` now
steps whole frames because acquisition takes one extra turn.

### F-AI-13 (S3) — dispatcher order and split passes
**VA** `0x421B50` · `PARITY-DEVIATION` · `ev_f6e88057…`
**Original.** One pass, groups 0..199 then members in order; per member:
idle (`0`/`6`) → arrival check → `FUN_0041435C(idle)` → gate → `FUN_0041EFD8`
→ `FUN_0041ECC8`; walk (`1`/`7`) → facing/queue commit → `FUN_0041F048` order
gate → arrival/pickup (`FUN_004110EC`) → `FUN_00421964`.
**Remake.** `combat_tick` runs staged passes: projectiles, 16-frame block, death,
equipment refresh, then the AI frame (group 0 first, then object/citizen groups),
then `update_world_combat` for movement and attack resolution.
**What now matches.** The *decision* order is per entity: `AiSchedule` gates each
unit (F-AI-04), group 0 is served before the other groups, and armed → retaliate →
follow/stance is applied per unit in that order (F-AI-08/F-AI-12). Acquisition,
retaliation, recall and the citizen stance gate are all resolved inside that turn.
**Recorded deviations (each needs its own trace, so this is a documented
`PARITY-DEVIATION`, not a fix):**

1. movement/animation (`update_world_combat`) runs once *after* every decision,
   instead of interleaved inside each unit's branch;
2. the citizen AI (`update_population`, `FUN_004168A4`) and the follower pass
   (`update_party`, `FUN_0041F3C8`) run on their own accumulators rather than
   inside the gated turn — their RNG draws are therefore correct in count but not
   interleaved with combat draws;
3. arrival/pickup (`FUN_004110EC`, `FUN_00421964`) lives on the route/click path
   (F-AI `world_items`) rather than in the dispatcher's walk branch.

Closing (1)/(2) means rewriting the movement passes with no runtime oracle; the
report's recommendation is to do it as its own wave, not as a tail-end patch.
**Sub-item — `0x4101E4` (approach offset): fixed as F-AI-14** (it is the
post-attack step-aside of `FUN_004123BC` case 5, exclusive to archetype `0x49`).

### F-AI-14 (S2) — post-attack step-aside is exclusive to archetype `0x49`
**VA** `0x4101E4` · `GAP-MISSING` · `ev_e23c7e36…`
**Status: fixed 2026-11-18** (`app/combat/approach.rs`,
`unit_ticks.rs::post_attack_reposition`).
**Original.** `FUN_004123BC` case 5 (melee resolution) ends with:

```
if (archetype != 0x49 || FUN_004101E4(self, &dx, &dy) == 0 ||
    FUN_004142D4(self, cell.x + dx, cell.y + dy, 8) == 0)
{ FUN_0041435C(self, *self); }
```

The `||` short-circuit is the whole story: every archetype except `0x49` takes
the first operand and simply advances its action. **Only `0x49`** probes and
moves. `rand()%4` picks the quadrant, then the free axis slides while the cell is
occupied, and the probe returns `0` when the slide reaches the positive bound:

| draw | offset | slide while occupied | fails when |
|---|---|---|---|
| 0 | `(-2·20, -9)` | `dy++` up to `+9` | `dy == 9` |
| 1 | `(+2·20, -9)` | `dy++` up to `+9` | `dy == 9` |
| 2 | `(-20, -2·9)` | `dx++` up to `+20` | `dx == 20` |
| 3 | `(-20, +2·9)` | `dx++` up to `+20` | `dx == 20` |

`_81D6E0 = 0x14 = 20`, `_81D6A4 = 9` at 800×600 (`FUN_0042781C`), the same
globals as the citizen wander radius. Mode 8 writes the whole order byte, so a
successful step-aside drops target, engage bit and ranged order; the unit
re-acquires on a later AI turn. A failed probe or unreachable goal just advances
the action.
**Remake before the fix.** The case-5 tail was absent: an `0x49` actor never
stepped aside and never drew that quadrant.
**Applied.** `approach_offset` implements the quadrant table, slide and bound
test; `post_attack_reposition` is called from the `attacks` resolution loop only
when the target **survives** (a kill breaks out of the case before the tail) and
returns immediately for any archetype other than `0x49`.
**Verification.** `quadrant_table_matches_the_original` (exact offsets, one
draw), `the_free_axis_slides_past_occupied_cells`,
`a_blocked_slide_returns_none`,
`post_attack_reposition_issues_a_far_step_aside`,
`post_attack_reposition_skips_other_archetypes`.
**Caught by the suite.** The first implementation read the `||` chain inverted
(probe for everyone *except* `0x49`); `idle_party_retaliates_against_adjacent_attacker`
went red because the ant stepped away instead of staying in contact. The tests,
not the decompiler prose, settled the polarity.

---

## 4. Sigmar verdict

`Game.1` entity 539 (Сигмар, group 65, `archetype 0x05`, `kind 6`,
`flags_a 0x40`, `raw_0x12 2`, `equipment [—,—,745,746,747,748]`, no ammo,
melee/archery mastery 700):

* He is **not** affected by F-AI-02/F-AI-03 (no bow, `raw_0x12 = 2`). Those two
  are still real for every archer in the game.
* He **is** affected by:
  * **F-AI-01** — after recruitment (`0x123` sets `+0x02 = 0x60`) he follows with
    the wrong formation and no RNG draw;
  * **F-AI-04** — his AI tick competes for the 3-action budget, which the remake
    does not model;
  * **F-AI-05** — retaliation triggers under different conditions;
  * **F-AI-08** — follow runs on a separate timer instead of behind the
    armed/retaliation/order-bit gate;
  * **F-AI-09/F-AI-13** — re-acquire state and acquisition order.

So "Sigmar's battle AI is inconsistent with the binary" is explained, but not by
one bug: it is the formation/RNG divergence plus the missing scheduler and the
retaliation gate.

**Status after the fixes (2026-11-18):** F-AI-01, F-AI-04, F-AI-05 and F-AI-09
are fixed, and F-AI-08 is fixed for the persistent-order case. Sigmar now picks
formation slots from the authored table with the shared RNG, takes his AI turns
through the three-action scheduler, retaliates only under the original's
predicates, drops the ranged flag on a re-acquire, and does not follow while he
holds an engage order. What remains for him is F-AI-13: the follower pass still
runs on its own timer instead of inside the dispatcher turn, and the merged
per-entity order is approximated by "group 0 first, then world".

---

## 5. Proposed fix order

| step | items | why first | risk |
|---|---|---|---|
| 0 | ~~F-TR-01~~ | **done** — one constant ×2, every price in the game | very low |
| 1 | ~~F-AI-02, F-AI-03~~ | **done** — 4× range error, single-slot predicate; localised to `los.rs`/`ai.rs`/`acquire.rs` | low — one constant + one index |
| 2 | ~~F-AI-01~~ | **done** — biggest behavioural + RNG divergence | medium — new constant + test |
| 3 | ~~F-AI-05, F-AI-08, F-AI-09~~ | **done** — party/companion gate chain, closes Sigmar (follower timer remains under F-AI-13) | medium — touches the tick order |
| 4 | ~~F-AI-04~~ | **done** — scheduler; cadence + RNG draw budget | high — needed the cursor semantics in tests |
| 5 | ~~F-AI-06, F-AI-12~~ | **done** — object AI branches (recall wander + archetype short-circuit) | medium |
| 6 | ~~F-AI-07~~, ~~F-AI-10~~, ~~F-AI-11~~, ~~F-AI-14~~, F-AI-13 | F-AI-07/10/11/14 **done**; F-AI-13 is recorded as a deviation (staged movement/citizen/follower passes) | medium |

Acceptance per item: ledger `fix_status = fixed`, a unit test that encodes the
original constant/branch, `cargo test`, `cargo check --all-targets`, and the
relevant smoke (`mob_ai_smoke`, `combat_smoke`, `ranged_smoke`,
`feedback_smoke`).

---

## 6. Wave B preview — trade pricing has a 4× coefficient error

### F-TR-01 (S1) — sell/buy skill coefficient is 0.002, not 0.008
**VA** `0x41AE00`, `0x41B5A8` · `GAP-VALUE` · `ev_4b3da54b…`
**Status: fixed 2026-11-18** (`web/src/trade.rs`, `web/src/trade/tests.rs`).

Assembly (`0x41AE00`): `FMUL double ptr [0x450024]`, `FMUL double ptr [0x45002C]`,
`FMUL double ptr [0x450034]`; (`0x41B5A8`): `FMUL double ptr [0x4500C4]`,
`FMUL double ptr [0x4500CC]`. The doubles decode as:

| address | value | role |
|---|---|---|
| `0x45002C` | 0.5 | sell base factor |
| `0x450024` | **0.002** | sell skill coefficient |
| `0x450034` | 2/3 | sell factor when `_DAT_0081D63C > 0` |
| `0x4500C4` | **0.002** | buy skill coefficient |
| `0x4500CC` | 1.5 | buy factor when `_DAT_0081D63C > 0` |

So the original is:

```
sell = round(total * 0.5 * (1 + (own_trade - npc_trade) * 0.002))   * 2/3 if role_mult
buy  = round(total * (1 - (own_trade - npc_trade) * 0.002))         * 1.5 if role_mult
```

`trade.rs::sell_value`/`buy_value` **used** 0.008 in both places before the fix.
With trade skills in `0..500`, the difference was large: at a ±250 skill gap the
original swings the price by ±50 %, the old code by ±200 % (clamped only by the
final round).

**Impact.** Every purchase and sale in the game is mispriced; settlement
merchants and the trade-skill progression (`(table_money + sell_value)/1024`)
inherit the error.

**Why it was missed.** `trade/tests.rs` uses equal trade skill on both sides
(`diff = 0`), so the coefficient never enters the expected values. The tests
encode the *intent* of the exchange flow, not the original arithmetic.

**Applied.** `trade.rs` now carries the four constants as named recovered
values (`TRADE_SKILL_COEFFICIENT = 0.002`, `SELL_BASE = 0.5`,
`SETTLEMENT_SELL_FACTOR = 2/3`, `SETTLEMENT_BUY_FACTOR = 1.5`) with the VA of
each double in the doc comment. New tests pin the coefficient
(`1000` base, skills `20` vs `10` → sell 510 / buy 980; `±250` → 750 / 500;
settlement multiplier applied after the skill term) and the rounding mode
(`5 → 2`, `3 → 2`, `1 → 0`).

**Rounding.** `FUN_0043EAAE` is `ROUND(ST0)` = x87 `FISTP` with the default
control word, i.e. round-half-to-**even**. Both price functions now use
`f64::round_ties_even()` (as `sound/position.rs` and `combat/los.rs` already do)
instead of `f64::round()`, which rounds half away from zero.

**Verification.** `cargo test` 530 passed (the two new price tests included),
`cargo check --all-targets` and `cargo check --target wasm32-unknown-unknown`
clean. `tools/trade_smoke.mjs` asserts only the direction of the money change,
so it needs no new expectation; it still has to be re-run after the next wasm
rebuild.

**Not ported (unreachable).** Both originals start with `if (_DAT_0081D574 == 0)
return total;` — no trade partner, price returned unrounded. A remake exchange
always has a partner entity, so there is nothing to map it to; if a future
"value display without an open trade" path appears, it must copy that early-out
rather than pass `npc_trade = 0`.

---

## 6.2 Wave B — trading (audited 2026-11-18)

| VA | verdict | what was checked |
|---|---|---|
| `0x431E2C` | PARITY | open: `+10` per class-`0x0B` subtype-`0x20` item, `99999` for merchants, role from `FUN_0041182C`, `role_sign` = `±role` from `FUN_0043425C` (`Settlement+0x4B2 & 0x10`), stock rows 24/32/23 with the `class ≤ 2` filter when `0x4B2 & 4` |
| `0x430D6C` | PARITY | balance: `sell = FUN_0041AE00(offer)+table_p` vs `buy = FUN_0041B5A8(stock offer)+table_n`, money moved **one coin per loop** in the original branch order — `rebalance` matches exactly |
| `0x4203F4` | PARITY | confirm/cancel: base weight measured with the inventory cleared, `+ Σ item weight` of bag + chosen offer row; `≤ 32` slots; `(endurance_base*1000)/3+10000`; confirm checks money then `money += table_partner − table_player` clamped at 0; refill from the offer row **then** the bag, skipping zeros; partner gets the other row; trade skill `+ (table_p + sell)/1024` for **both** sides capped 100 |
| `0x42462C` | PARITY | front insert: capacity scan, then shift `0..30 → 1..31` and write slot 0 (`insert_inventory_front`) |
| `0x41B7E4` | PARITY | weight = equipment + additional + inventory + special, class-12 stacks × quantity; `trade_base_weight` reproduces the clear-measure-restore trick |
| `0x41EAD8` | PARITY-DEVIATION | slot occupied = `!= 0 && != _DAT_0081D670` (the held/cursor item); the remake keeps the held item out of the slot arrays, so the sentinel branch cannot occur |
| `0x421AD0` | PARITY | remove the first slot whose id matches, shift down, clear slot 31 (`remove_inventory_id`) |
| `0x4245B4` | PARITY | *wake helper*, not a name refresh: `group+0x1D = 1` and `+0x14 = 0xFF` for members with `flags_a & 1 == 0` and `archetype != 0x49` (`wake_group`; the remake also resets walking/target) |
| `0x417010` | PARITY-DEVIATION | opens the ground-container window (record index, `+0x14` 22 ids into row 2, no NPC); the remake collects ground items by right-click/walk-over |
| `0x42BEA4` | PARITY | trade row strip draw: origin table per resolution, INTR18/19 caps, page offset — `trade_screen.rs` matches (visual only) |
| `0x4106E0` | PARITY | *orb/key teleport destination*, not "add quest item": `FUN_00414DE4` cases 14–19/21 call it with `(location, x, y, facing)`, writing `_81D6D8 = −location` and the party arrival cell; `class11_orb` carries the identical table `(28,184,18,1) (5,60,68,2) (10,147,66,1) (20,49,41,2) (3,141,54,1) (40,42,31,1) (43,18,24,2)`. Case 20 is the map orb (mode 4) → `class11_map_orb` |
| `0x41079C` | **GAP-MISSING → fixed** | XP grant + AUTO auto-spend, see F-TR-02 |

### F-TR-02 (S1) — kills award no XP and AUTO spends nothing (`0x41079C`)
**VA** `0x41079C` · `GAP-MISSING` · `ev_7e6f719a…`
**Status: fixed 2026-11-18** (`character/xp.rs`, `app/combat/xp.rs`,
`melee.rs`, `ranged.rs`).
**Original.** Despite the CSV label "add money to entity", this is the XP grant:

```
available_xp(+0x38) += amount
if (Entity+0x03 & 2) {                       // AUTO
    loop {
        r0 = FUN_00410B08(e,0); if (r0) level(+0x33) += 1
        r6 = FUN_00410B08(e,6); if (r6) { xp -= r6; invested(+0x3C) += r6; +0x64 += 1 }
        r5 = ... (5); +0x60 += 1
        r2 = ... (2); +0x56 += 1
        r3 = ... (3); +0x5A += 1
        r1 = ... (1); ...
    } while (any row was affordable)
    if (changed && e == leader) refresh the party screen (and leave the character screen if open)
}
```
Rows map to level, endurance current (`+0x64`), strength current (`+0x60`), dexterity current (`+0x56`), charisma current (`+0x5A`) and the sixth stat.

`FUN_004123BC` case 5 calls it with `power/4` on a kill and separately adds `power` to **melee mastery** (`+0x40`). The ranged path already grants `power` archery mastery, so the mastery half exists; the XP/auto-spend half does not.

**Impact.** No kill ever granted experience, so levels/stat raises only came from quest opcodes (`+10/20/50`); the AUTO button toggled a bit that nothing read.

**Applied.** `character/xp.rs::grant_xp(XpView, amount)` is the engine: signed
grant to `available_xp`, then (AUTO set, no temporary effect, resolvable class
index) a loop over `can_raise_any_index` (level row) and
`raise_cost_index` rows **endurance → strength → dexterity → charisma**, spending
into `invested_xp` and bumping the stat, exactly like the original's
`do { … } while`. `app/combat/xp.rs` adapts it for the leader, companions and
world actors and adds `award_kill_reward(owner, power, melee)`:
`power / 4` XP plus `power` into `melee_mastery` (`+0x40`) or `archery_mastery`
(`+0x44`). `resolve_attack` calls it on a lethal melee hit (corpse loot stays
`kill_combatant(defender, 0, 0)`), and `award_ranged_kill` now pays XP instead of
the old `power/4` money. `RANGED_KILL_MONEY_DIVISOR` became `KILL_XP_DIVISOR`.

**Verification.** `character/xp.rs` unit tests (no-AUTO grant, AUTO spend with
`invested + available` conserved, temporary-effect block, negative grant);
`combat_ai.rs::kill_rewards_pay_experience_and_mastery_not_money` and
`auto_spends_the_kill_reward`; `cargo test` 562, `combat_smoke`, `ranged_smoke`,
`mob_ai_smoke` and `smoke` all pass.

**Still not wired.** The other `0x41079C` callers (`0x414DE4` item/spell use,
`0x41DAB0` item use, `0x410010` amulet combine, `0x4345E0` quest opcode) do not
grant XP yet; the kill paths were the ones the audit could pin exactly.

---

## 6.3 Wave C - settlement economics (complete)

`0x41D048` is not a "death/action transition" (CSV again). It is the **whole
16-frame periodic pass**, alternating two phases on `_DAT_0081D640`:

* **phase A** `(_81D640 & 0xF) == 0` — per-entity: periodic damage (`+0x50` →
  `+0x5C`, recompute `FUN_0041C69C`, death row + `FUN_00418480(entity, 0,
  leader_level)`), the **temporary-effect countdown** (`+0xCE`, restore
  `+0xBC..+0xC2`), party marker refresh; object groups → `FUN_00412C30` +
  `FUN_0041BD3C`.
* **phase B** `(_81D640 + 7 & 0xF) == 0` — the 20-settlement pass: worker count →
  no-worker clear → power-ratio defeat → claim → money event → trade/training.
  The defeat ratio constant `0x4501A8` decodes to **2.0**.

Verified so far: the settlement order matches `tick_settlement_pass`
(no-worker clear → defeat → claim → random event → trade → training), the worker
gate is `visual < 3` + visible, and the phase-A poison/death call passes the
**leader's** level as the corpse-loot tier.

### F-SET-01 (S2) — temporary effects expired 16× too fast
**VA** `0x41D048` (phase A) · fixed 2026-11-18 (`app/inventory.rs`,
`app/combat/tick.rs`, `app.rs`).

The original decrements `Entity+0xCE` **once per 16-frame pass** (~0.96 s); a
potion sets it to `amount * 0x1e` (30 frames), so a one-unit draught lasts
~28.8 s. The remake set the same value — `SetTempTimer(local * 30)` — but
decremented it on every 60 ms tick, i.e. ~1.8 s and 16× too fast, which also made
`FUN_00410B08` (which refuses while `+0xCE != 0`) stop blocking far too early.

**Applied.** `tick_temp_effect_pass()` replaces the per-tick accumulator and runs
inside the 16-frame block next to `apply_dot`; it ticks the leader, the party and
every actor of a **non-object** group on the current location (phase A skips
object groups). The now-unused `temp_effect_accum` field is gone.

**Verification.** `app/tests/inventory.rs::temporary_effect_restores_saved_stats`
steps one pass instead of one second; `cargo test` 562, `combat_smoke`,
`ranged_smoke` and `smoke` pass.

**Wave C is complete** (C1–C5 below): every settlement row has a verdict —
10 parity, 2 documented deviations, 5 fixed. The set-bonus dependency
(`FUN_0041C69C`) is tracked with the COMBAT wave.

### C2 verification result (2026-11-18)

* **Worker count / no-worker clear — PARITY.** The original counts `local_24` =
  every visible `visual < 3` member and only *afterwards* runs
  `local_24 -= apprentices + waypoint_owners` (line 248), i.e. the clear tests
  the **pre-subtraction** count. `SettlementWorkers` keeps exactly that split:
  `free` (pre-subtraction, used for the clear) and `workers` (post-subtraction,
  used to scale target timers).
* **Defeat check — PARITY.** `flags_4b2 & 0x80` skip, `activity == 1` gate,
  party power = round leader + companions, defenders = visible `visual < 3`
  non-death-row members, `party / defenders >= 2.0`; the `defender == 0` case is
  guarded instead of dividing by zero.
* **Event timer — PARITY.** `progress_14 += 1`, fires at `>= 0x2760`
  (`0x275F < +0x14`), skipped while `flags_4b2 & 0x10`.
* **Money-event formula — PARITY (assembly-pinned).** From `0x41D81D…0x41D878`:

  ```text
  FILD settlement.byte[+0x00] * 20
  FILD leader.trade(+0x42); CALL 0x43EB2E (sqrt); FMULP
  FILD group.entity_count(+0x19) * 5; FADDP
  FLD1; FADDP
  FILD settlement.pool(+0x10); FADD
  CALL 0x43EAAE (round); FISTP settlement.pool(+0x10)
  ```

  i.e. `pool = round(pool + 1 + members*5 + settlement.byte0*20*sqrt(leader.trade))`,
  then `+0x14 = 0`. The remake computes `round(members*5 + target_base0*20*sqrt(trade) + 1)`
  and adds it to `money_pool_10` — identical, because the pool is an integer
  (`round(pool + x) == pool + round(x)`). Its only difference is the documented
  `authored_count − party_in_group` compensation for recruited members that still
  own their source row.

C2 is therefore **fully parity**; no code change was needed.

### C3 verification result (2026-11-18)

* **Waypoint claim `0x416670` — PARITY.** Gate `flags_4b2 & 5`; scan the party
  group's members (leader first) inside the settlement group's rectangle A;
  visible only; `flags_4b2 & 1` claims directly, otherwise the member's
  `visual_index` must differ from `(settlement+0xAA) & 3` **and** it must carry
  equipment or a class-`< 3` inventory item; on a match the group gets its target
  grouping and `+0x1D = 1`. `settlement_claim`/`claimable` match, including the
  leader-first order and the early return.
* **Target/prop state machine `0x4172AC` — mapped, not yet verdicted.** The
  record index is `settlement.group_id * 0x24`; a timer array at `0x81B954` is
  decremented; the object id comes from the `0x458A02` table (stride `0x40`) and
  the prop record at `0x75FCB4 + id*0x100`; per-part offsets roll
  `rand()%span` from `0x45AE68`/`0x45AE6C` into `+8+i`; a `-1` timer becomes
  `-2`, pushes the index onto the `0x81C194` list and sets `*record |= 0x80`
  (hide). `settlement_targets.rs` implements the pass, but the tables and the
  `-1 → -2` list need decoding before a verdict.

### C4 verification result (2026-11-18)

* **Barracks training `0x417F84` — PARITY** (verdict recorded). The `0x2D0`
  timer, the `visual < 3` member filter, the `FUN_00410B08` level-up gate, the
  per-stat `+1000` temporary XP trick and the mastery gain all match
  `settlement_training.rs`.
* **Role-2 restock `0x4175FC` — GAP-VALUE, fixed.** The remake allocated the
  raw subtypes `2/0/1` (herbs) with quality `1.0`. The original allocates those
  subtypes and then runs `FUN_0041BB70` against a synthetic stack target
  `{class 9, subtype 3}`, i.e. rows `#25 (2,3)->5`, `#21 (0,3)->6` and
  `#23 (1,3)->7` of `0x450DF4`: the *product* is subtype `5/6/7` (healing
  potion, poison, weapon poison), quality `healing * 0.01` capped at `15`,
  and the producer gains the row's healing skill (`0/3/2`). **Applied:**
  `inventory::craft_restock` performs that craft on the freshly allocated
  instance, the quota is only spent on success (a full table aborts the pass
  like the original's `return 0`), and the production order means the three
  qualities are `201*0.01`, `201*0.01`, `204*0.01` for the loc-1 merchant
  (healing `201`). The six combine rows and the three quota words were already
  parity. One invisible deviation remains: the original also writes the 25th
  element (`i = 27`, `+0x442`), but `FUN_00431E2C`
  copies only 24 entries from `+0x412`, so the remake's 24-slot row dropping
  it cannot be observed.
* **Role-4 restock `0x4179C8` — GAP-VALUE, fixed.** Slot selection, the
  `FUN_00417190` kit, the recipe-level tables, the item classes/subtypes and
  the durability source (`u16@+0x2E`), the 23-slot row and the
  redistribute-to-a-group-member branch all match. Two values did not: the
  remake produced after one pass (`timer4 = 1`) and raised smithing by `1`.
  **Applied:** `role4_delay` = `round(level*60 / sqrt(smithing))` (x87 `FISTP`
  of `+inf` when the skill is `0` stores `INT_MIN`, and the wrapping `DEC`
  turns it into `INT_MAX`, so a skill-less producer never completes — faithful
  to the original) and `role4_grow` =
  `round(sqrt(smithing^2 + q/24))` (`0x45001C = 1/24`, `q` = the recipe level
  for ammo, otherwise `round(1 + quality/sqrt(smithing))`; a zero skill leaves
  `0`). The loc-1 smith (`smithing 9`) now reserves slot `0` for
  `round(60/3) = 20` passes instead of one.

**Verification.** `app::tests::trade::role2_restock_crafts_potions` (subtypes
`5/6/7`, qualities `2.01/2.01/2.04`, healing `201 -> 206`),
`role2_and_role4_restock_produce_stock` (role-4 reservation + the `20`-pass
delay), `app::settlement_trade::tests::{role4_delay_matches_the_x87_formula,
role4_grow_uses_the_quarter_day_sqrt}`; `cargo test` 565, `trade_smoke`,
`smoke` and `save_smoke` pass.

### C5 verification result (2026-11-18)

Every remaining settlement row is now audited (17/17). Ten are parity, two are
documented deviations and three gaps are open.

| VA | verdict | note |
|---|---|---|
| `0x41093C` | PARITY | herb respawn: sole class-9 subtype 1/2 item, full 16-byte clone, `-frame` timestamps, `+0x12 = 0` otherwise |
| `0x411078` | PARITY | `max_party` threshold table `0x45BC54` + the `if i < 10 { i += 1 }` rule |
| `0x41182C` | PARITY | `Settlement::role_of` (first five waypoint owners, slot + 1) |
| `0x4110EC` | PARITY-DEVIATION | pickup transfer order matches; no `FUN_00417010` discard window, ammo stacks on pickup |
| `0x411DE4` | PARITY | placement: centre/half, `rand % half`, random sign, 100 attempts |
| `0x416670` | PARITY | claim (C3) |
| `0x417144` | PARITY | `level_tier` (`<4 -> 3`, `<7 -> 6`, `<10 -> 8`, else `10`) |
| `0x4172AC` | PARITY-DEVIATION | prop rebuild semantics; cosmetic RNG replaced by a stable hash, no shared-record hide list |
| `0x417F84` | PARITY | training (C4) |
| `0x41B658` | PARITY | power = `armor*0.1 + dex*0.2 + str*0.3 + end*0.3 + melee*0.002 + archery*0.003` (+`rating*0.2` for slots 0/2) |
| `0x420A8C` | PARITY | growth constructor averages/pairs/`0x640` HP/placement |
| `0x4313B0` | PARITY | party skill scan for predicates `0x116..0x119` |
| `0x431520` | PARITY | max building skill for predicate `0x146` |

**Gaps found and fixed (C5).**

1. **`0x41D048` phase-B target loop (re-verified 2026-11-21) — the earlier
   inversion was itself a misread.** The outer guard is
   `if (key < 0 || timer > 0 || (state != 6 || workers < 1))`, so the reset
   `else` fires **only** for `key >= 0 && timer <= 0 && state == 6 &&
   workers >= 1` (`changed = 1; state = 0; timer = V1*60/workers`, then
   `FUN_004172AC`). A `timer <= 0` target with any other state executes
   nothing: an untouched foundation (`state 0`) waits for the build-menu
   timer write and a completed target (`state 3`) stays complete. The
   2026-11-18 change inverted this and reset every idle target, so the first
   pass after map entry collapsed every shipped `state 3` building (e.g.
   loc 4 «Старый лагерь») into its foundation. **Applied:** the reset is
   guarded by `state == 6 && workers >= 1`, the timer is read as a signed
   high word, `arm_timer` keeps the zero-divisor guard and wrapped quotient,
   and the fixtures cover ruined-rebuild, completed-target idling and the
   shipped loc-4 table.
2. **`0x41BD3C` member pass — `GAP-BRANCH`, fixed.** Its second half ticks
   poison and `+0xCE` for *every* member of *every* group with no location
   filter (only the death row/corpse is current-location gated).
   `tick_temp_effect_pass` now ticks the leader, the party and every actor of
   every cached population. Off-map poison/death remains a structural
   deviation: the per-map cache only simulates built locations.
3. **`0x418E6C` map-entry stock seeding — `GAP-MISSING`, fixed.**
   `app/settlement_stock.rs` ports the whole ladder: 1–3 jars into role-2
   elements 0 and 14 (`+0x412 + 0x1C*i`), role-3 slots 0–2 (class 6/7/8, 70%),
   3–4 (class 11, 50%), 5–7 (melee weapons, 30%), 8–13 (armor, 50%), then the
   fixed 18-item ladder 14–31 with the two `FUN_00417190` suits and the five
   `rand%2`/`rand%3` plain pieces. Conditions, `level/5`/`level/4` subtype spans,
   the `level_tier` condition rolls and the `+1`-less tens digits are preserved.
   Called from `ensure_population` with the location RNG. Role 3 is now a real
   merchant instead of a permanently empty window.

**Verification.** `cargo test` 567 (`target_pass_ignores_sentinels_and_resets_idle_targets`,
`building_progresses_construction_damage_and_rebuild_sprites`,
`temp_effect_pass_ticks_cached_world_actors`,
`map_entry_seeds_settlement_stock`); `feedback_smoke`, `ranged_smoke`,
`trade_smoke`, `save_smoke`, `smoke`, `combat_smoke`, `mob_ai_smoke` all pass.
The ranged prop harness now stages the target at state 3 (`debug_prop_setup`) so
the sprite cannot change mid-flight.

**Cross-reference.** `0x41B658`'s armor input also includes the set-bonus value
`+0xC4` (recomputed by `FUN_0041C69C`, which fills the `0x81B930`/`0x81CFB8`
set accumulators). That function is a COMBAT ledger row and is still unaudited;
no shipped entity carries `+0xC4`/`+0xD1`, so the term is currently 0.

---

## 6.1 Verified parity (no action)

| VA | function | verification |
|---|---|---|
| `0x420190` | nearest unengaged | taken-set = members of the caller group with `+0x02 & 1` whose target equals the candidate's one-based id; distance to the acquiring entity; matches `nearest_unengaged` |
| `0x420098` | plain nearest | visible/alive filter + `FUN_0043881C` distance to the acquiring entity; matches `nearest_enemy` |
| `0x412C30` | group relation pass | party-in-rectangle → `+0x1A = leader.group`, `+0x1D = 1`; own-member-outside → `+0x1D = 0x20`; else `0`; matches `update_group_relations` (remake hardcodes target group `0`; the original copies the leader's group byte) |
| `0x43EA46` | `rand()` | LCG `state*0x41C64E6D + 0x3039`, `(state>>16)&0x7FFF`; `combat::OriginalRng` identical, save-persisted |
| `0x429570` | *(misnamed)* MCI music open | platform-substituted by WebAudio; no gameplay logic |

---

## 7. Notes on adjacent systems (not yet audited)

1. `PLAYABLE_V*`/`AGENTS.md` say the line-of-fire probe is "a deterministic
   unit-step DDA". It is deterministic, but the step is **4 px**, not 1 px
   (§F-AI-02). The doc comment in `los.rs` must be corrected when the fix lands.
2. `KONUNG_SOURCE_OF_TRUTH.md` §25 #4 (RNG unknown) should be closed with the
   LCG identity; the remaining unknown is only the *seed source*/draw order.
3. `evidence/konung_res_functions.csv` should carry a `verified_role` column
   (or the ledger's `divergence` note) rather than being re-labelled in place —
   it is a historical artifact.
4. **Resolved (was an alarm).** While fixing F-AI-07 the ranged smoke's staged
   target appeared to advance two cells in one logic tick. It is one legal step:
   direction 6 in the isometric tables `0x48010C`/`0x480110` is `(+2, 0)`, so a
   single committed step changes x by two. There is no double committer.
   `app/tests/combat_ai.rs::approach_moves_at_most_one_direction_step_per_tick`
   now pins the invariant: each tick moves at most one direction-table delta.

---

## 7. Wave D — quests (D1 audited 2026-11-18)

Wave D is 66 rows: 13 core/helper functions and 53 opcode handlers
(`0x112..0x16E`). **D1 (core + helpers) is complete: 9 parity, 1 deviation,
3 fixed.**

| VA | verdict | summary |
|---|---|---|
| `0x434FE8` | PARITY | `B4` evaluator: `0x4000` terms AND while true, plain terms OR only after a failure (assembly `OR`, not assignment), `0x2000` = opcode term, `0x8000` negates, operand `& 0x1FF` |
| `0x434BB8` | PARITY | `B5` executor: `0x2000` runs the opcode, else `0x8000` sets / plain clears the `B7` bit |
| `0x434C74` | **GAP-BRANCH, fixed** | branch resolve + page build (see below) |
| `0x43159C` | PARITY | runner family split (`<0x38`/`<0x70`/`<0xA6`/`<0xDC`/`<0x112`) + the `0x465BD4` handler table; the 1-based low operands map to `(op-base)-1` |
| `0x435290` | **GAP-VALUE, fixed** | opener: template/voice selection (see below) |
| `0x430430` | PARITY-DEVIATION | tiles the 100x100 stone over the window; the `_81D614` filled/tiled toggle is not modelled (default tiled) |
| `0x435188` | PARITY | fragment join: a space before a fragment starting with a letter/digit/lone `-` |
| `0x433A8C` | PARITY | find class-`0x0B` bag item (`slot+1`, 32 slots, `subtype == operand-1`) |
| `0x43395C` | PARITY | remove it: destroy (`class 0xFF`), shift, clear the last slot |
| `0x4347D0` | PARITY | grant it: allocate, front-insert, else drop on the leader's cell |
| `0x432F8C` | PARITY | pending-spawn test: `count < target` or any hidden (`archetype & 0x80`) member |
| `0x433FF8` | PARITY | no-live-member test: absent group -> true; else no member with `archetype & 0x80 == 0 && row != 9/3` |
| `0x431654` | **GAP-VALUE, fixed** | station specialist: destination vs teleport (see below) |

**D1 gaps found and fixed.**

1. **`0x434C74` now requires a surviving option.** `build_page` returns `None`
   when the option list is empty (so `open_dialog` closes and skips the voice,
   exactly like `_81D6D0 == 0`), and an **unconditional** option with text
   `0xFFFFFFFF` is dropped (a conditional one is kept). The
   `+0x08 = _81D31C4C` default-highlight write remains cosmetic; the hop guard
   is a deliberate addition.
2. **`0x435290` template/voice.** `dialogue_exec::dialog_template` reproduces
   the selection: humanoids `rand()%4 + 0x0C`, re-rolled to `rand()%4 + 0x0D`
   (`0x10 -> 0x0C`) when equal to the previous template; object NPCs class
   `Q`/`R`/`S` -> `0x11`/`0x12`/`0x13`, else `0x10`. `PlayableApp.dialog_template`
   stores `_81D68C`, and the chosen id is queued as the NPC voice once the page
   resolves (the roll uses the sound stream). The window art still uses the
   single `INTR[1]`/`INTR[0]` layout (cosmetic).
3. **`0x431654` routes instead of teleporting.** The settle snapshots the live
   cell (party motion / `QuestContext.live_cells`) and keeps it, so the waypoint
   cell remains the AI destination and the specialist walks there; the waypoint
   facing write is gone (the citizen AI faces it on arrival).

**Verification.** `cargo test` 571 (`zero_option_page_does_not_open`,
`unconditional_option_without_text_is_skipped`,
`humanoid_template_range_and_draw`,
`object_template_is_fixed_and_consumes_no_rng`); all headless smokes pass
(`feedback`, `smoke`, `save`, `trade`, `combat`, `ranged`, `mob_ai`), wasm
rebuilt.

Cross-references already audited: `0x4313B0`/`0x431520` (C5 parity) back the
skill scan and building threshold used by `0x116..0x119`/`0x146`;
`0x431E2C` (trade) is Wave B.

### D2 verification result (2026-11-18)

The 22 handlers in `0x110..0x13F` are complete: **21 parity, 1 fixed**.

| VA | opcode | verification |
|---|---|---|
| `0x431DCC` | 112 | `Group[npc.group].activity = 1`; `flags_4b2 |= 0x15` when the settlement group matches |
| `0x4346B0` | 114 | `Entity+0xD3 = 0x46` |
| `0x433F28` | 115 | `rand()%100 < 50` |
| `0x4337A8`/`0x432F10`/`0x4335EC`/`0x4330C4` | 116–119 | `FUN_004313B0(4/1/3/2) != 0` (helpers parity from C5) |
| `0x433824` | 11C | `flags_4b2 |= 0x20`, money +100, **returns 0** |
| `0x433868` | 11D | money −5, returns 1 |
| `0x431820` | 123 | recruit: role gate `Entity+0x04 & 0x38` (assembly), the three apprentice gains, group-0 move `+0x05=0`/`+0x02=0x60`/`+0x14=0xFF`, own waypoint clear |
| `0x43453C` | 124 | class 8 subtype 0x12 into the first empty bag slot, no drop fallback |
| `0x43425C` | 12E | settlement exists && `flags_4b2 & 0x10` |
| `0x433760` | 130 | voivode `+0x3F2` != 0 |
| `0x4330F8` | 131 | money > 29 |
| `0x4331FC` | 136 | `500/300` by NPC level > 3, halved unless `flags & 0x10`, compared with money |
| `0x4336E8` | 138 | settlement exists |
| `0x432F44`/`0x433718`/`0x4337DC` | 13B/13C/13D | target rows **1/4/6** state == 3 (`+0x21`/`+0x39`/`+0x49`) |
| `0x4334D8` | 13E | bag has class 9 subtype 2 |
| `0x4335A4` | 13F | role-4 producer `+0x3EA` != 0 |

**F-QS-04 — `0x12B` granted XP without the AUTO spend — fixed.** The handler
calls `FUN_0041079C(leader, 10)`, the XP engine: `available_xp += 10`, then —
while `Entity+0x03 & 0x02` (AUTO) is set and no temporary effect is active — it
spends the pool through the `FUN_00410B08` stat/level raisers. The arm only
added 10 to `available_xp`, so an AUTO-flagged leader banked quest XP. **Applied:**
`0x12B`/`0x12C`/`0x12D` go through `QuestContext::grant_player_xp` →
`character::xp::grant_xp` (the adapter kill rewards use) and still return 1 when
nothing is spent. `0x12C`/`0x12D` (`LAB_00434610`/`LAB_00434640`, not ledger
rows) call the same engine with 20/50.

`app::tests::ops::xp_opcodes_route_through_the_xp_engine` verifies the banked
case and the AUTO case (`available_xp + invested_xp` is conserved across the
spend); `cargo test` 572, wasm rebuilt, `smoke`/`save`/`trade`/`feedback`
pass.

### D3 verification result (2026-11-18)

The 21 handlers in `0x141..0x15F` are complete: **13 parity, 8 fixed**.

| VA | opcode | verification |
|---|---|---|
| `0x43307C` | 141 | role-3 producer `+0x3E2` != 0 |
| `0x43355C` | 142 | target row 2 state == 3 |
| `0x432D28` | 145 | signed `Entity+0xD2` >= 0 |
| `0x432DE8` | 146 | settlement group max building (`0x431520`) >= 5 |
| `0x432E48` | 147 | `target_base1 != 0` and trailing `base0+7` has a non-zero state or timer |
| `0x433620` | 149 | bag has class 9 subtype 0 |
| `0x433EFC` | 14B | `FUN_0041BA08`: 6 equipment + special + 32 bag + 5 additional, clear bit 0 of `+0x0E` and bank identification, clamp 100 |
| `0x432954` | 14C | settle into the village group + waypoint 0 (D1 fix keeps the live cell) |
| `0x433FD4`/`0x433FB0`/`0x433F8C` | 159/15B/15C | `FUN_00431654(4/3/2)` stationing |
| `0x434168` | 15D | `max_party <= group0 count` (leader + companions) |
| `0x434228` | 15E | NPC group id == 0 (party overlay) |
| `0x433BA0` | 14F | **GAP-VALUE, fixed** group heal (below) |
| `0x432544`/`0x4326A4`/`0x432754`/`0x432314`/`0x4325F4`/`0x4323C4`/`0x432474` | 151–157 | **GAP, fixed** build-target predicates (below) |

**F-QS-05 — `0x14F` group heal — fixed.** For every visible party-group member
(leader then companions) the original does

```
NPC.healing(+0x48) = round((1600 - member.hp) * 0.015 + NPC.healing)
member.hp = 1600
```

with the constant at `0x4505F0` = **0.015** and the skill accumulated on the
**healer NPC**. `0x14E` (`0x433B18`, not a ledger row) is the leader-only form
with the same constant. **Applied:** `heal_target` accumulates
`round_ties_even((1600 - hp) * 0.015 + healing)` on the NPC, leader first then
visible companions, and restores each unit to 1600; the old
`(640 - hp)/1024 + base` per-member write is gone.

**F-QS-06 — `0x151..0x157` build predicates — fixed.** Each handler reads its
target's key byte as a **signed** char and returns false when it is `>= 0x80`
(the 0xFF padding rows, present in 10 settlements per shipped save);
`requirement_predicate_keyed` now rejects those before the row lookup.
`0x155`'s original quirk is preserved: it takes the requirement key from
**target 1** while checking target 2's state/timer (`keyed(2, 1)`).

**Verification.** `cargo test` 573 —
`healing_ops_target_the_protagonist_and_party` (exact accumulated healer skill)
and `build_predicates_reject_padding_keys_and_0x155_uses_target1_key`
(0xFF rejection for targets 1/4, the 0x155 key source); wasm rebuilt;
`smoke`/`save`/`trade`/`feedback`/`combat` pass.

### D4 verification result (2026-11-18)

The 10 handlers in `0x160..0x16F` are complete: **8 parity, 1 deviation,
1 fixed**. With D4, **Wave D is 66/66 audited and 0 open**.

| VA | opcode | verification |
|---|---|---|
| `0x4342A4` | 160 | the **dialogue NPC's** level > 3 (also `FUN_004342A4` for 0x136) |
| `0x4321C4`/`0x43226C`/`0x4322C0`/`0x432218`/`0x4320CC` | 163/164/165/167/168 | target 1/4/6/2/5 timer = the key row's `+0x06` (`write_target_timer`) |
| `0x432120` | 169 | all `target_base1` trailing targets get that timer; the original reads the key signed and can index out of bounds |
| `0x433C94` | 16B | NPC leaves: clear/decrement waypoint owners, occupancy, group count, compact + re-stamp entities |
| `0x4341B8` | 16E | leader charisma base > 60 |
| `0x434364` | 162 | **GAP-VALUE, fixed** apprentice `flags_a` (below) |

**F-QS-07 — `0x162` apprentice battle stance — fixed.** The marker selects the
first eligible settlement member (visual < 3, no role bits, visible, no owned
waypoint), adds the producer role bit (`0x08`/`0x10`/`0x18`) and timestamp, then
does `flags_a (+0x02) = 0` before `FUN_00414A34` (which ORs `0x40`, plus `0x20`
only when the member shares the leader's group). For a settlement villager the
post-state is `flags_a = 0x40` — peace/idle. **Applied:** `flags_a = 0x40`, so a
candidate already in battle stance (`0x04`) no longer keeps it; the smith
apprentice test stages `0x44` and asserts `0x40`.

**`0x169` recorded as `PARITY-DEVIATION`.** Its key read is signed with no
guard, so the 0xFF padding keys index `0x458A06 - 0x40` and arm the padding
target with a neighbouring table's word. The remake writes 0 (idle); emulating
the out-of-bounds read is not sensible.

**Verification.** `cargo test` 573 (`app::tests::party` smith-apprentice flow,
now asserting the peace flags); wasm rebuilt; all headless smokes pass.

---

## 9. Wave E — items and character (E1 audited 2026-11-18)

Wave E covers the item/character **model** (not the damage rolls, which stay in
A2, and not the cursor/renderer, which stay in F). Proposed slices:

| slice | content |
|---|---|
| E1 | item data: icon, trade value, weight, requirements, stock combine |
| E2 | equip/held: auto-equip, slot equip, held-item pickup/drop, merge/consume, release refs |
| E3 | use/craft: `0x41DAB0` class dispatch, `0x41E160`, `0x41BB70`, `0x414DE4`, amulet parts |
| E4 | character: `0x410B08` cost, `0x41AB7C` armor, `0x41C69C` set bonuses, `0x41BA08` identification, `0x42F5F8` tooltip |

**E1 (5 rows): 3 parity, 2 deviations.** The CSV roles of three rows were
wrong:

| VA | real function | verdict |
|---|---|---|
| `0x42BA40` | item **icon** selector (four class tables, `+0x02`) | parity |
| `0x41B2A8` | item **trade value** (price + condition bonus, class 12 × qty, class 9 float charge ×8) | parity |
| `0x41B0C4` | item **weight** (class 12 × qty) | PARITY-DEVIATION: class 10 returns 0 in the original, table weight here; no shipped class-10 instance |
| `0x4182D4` | equip requirement predicate | PARITY-DEVIATION (below) |
| `0x4171D8` | role-2 **stock combine** (not “merge stacks”) | parity (Wave C) |

**`0x4182D4` is behaviourally parity.** Its condition-record branch
(`u8@0x458533 + cond*0x10`, stat `u8@0x458532`) is dead in the shipped data —
all 18 records at `0x458530` store `+0x02 = +0x03 = 0` — and the prototype
requirement fields exist only in the stride-`0x40` records (classes `0..5|12`),
which the remake returns as 0 for the other classes anyway. `_81D638` (the
debug bypass) has **no writer**, so it never triggers. `requirement_ok`'s
prototype-only check is therefore equivalent; the missing code paths are
recorded, not guessed at.

**Deferred.** `0x42F5F8` (the full item tooltip formatter, 11.6 KB) is assigned
to E4 with the character screen; the remake's `app/tooltip.rs` is a
data-derived approximation, not the original string layout.

**E2 (7 rows): 2 parity, 2 deviations, 3 fixed.**

| VA | function | verdict |
|---|---|---|
| `0x4104D4` | auto-equip one main slot (class == slot, requirement, consume) | parity |
| `0x4105A0` | equipment refresh (ammo first, slots 0/1/3/4/5/2, `+0x12` rule) | PARITY-DEVIATION: `_81D5F0 += 1` not charged (F-AI-12) |
| `0x41E450` | equip into a main/ammo slot | **GAP-BRANCH, fixed** |
| `0x41E6BC` | equip into an additional slot (class 6/7/8 map) | parity (shares the full-bag note) |
| `0x41E8A8` | held-item put-down/pick-up | **GAP-BRANCH, fixed** |
| `0x422520` | release an entity's item references | **GAP-MISSING, fixed** |
| `0x424410` | give the held item to an actor | PARITY-DEVIATION: ammo stacking addition |

**F-IT-02 — `0x41E450` ammo-slot class gate — fixed.** Param 12 requires
class 12 (`class != param` rejects). `place_held` now gates
`ItemLocation::Special` with `class == 12`, so a sword can no longer occupy the
`+0xCC` ammo word and make the bow look active. (The handler also ignores a
full-bag insert failure and loses the displaced item; the remake restores it —
a deliberate, safer deviation.)

**F-IT-03 — `0x41E8A8` put-down restores the source container — fixed.**
`deposit_held` writes the held item back into `HeldItem.source` when the target
is its own owner and that slot is free (equipment, additional, ammo or bag
index); other owners and free-form grants still front-insert (`0x424410`).

**F-IT-04 — `0x422520` release a departing NPC's items — fixed.**
`remove_world_entity` marks the special, 6 equipment, 32 bag and 5 additional
instances `0xFF` before detaching (`0x16B`); `0x14C` settle keeps the gear.

**Verification.** `cargo test` 576 (`only_ammo_fits_the_special_slot`,
`put_down_restores_the_source_slot`,
`removing_an_actor_releases_its_items`, updated
`lift_place_and_deposit_equipment`); wasm rebuilt; all headless smokes pass.

**E3 (5 rows): 2 parity, 1 deviation, 2 fixed.**

| VA | function | verdict |
|---|---|---|
| `0x41DAB0` | class-9 potion use / oil / poison application | **GAP-VALUE, fixed** |
| `0x41E160` | remove one instance from a container (+ snapshot) | parity |
| `0x41BB70` | alchemy recipe table (33 rows) | PARITY-DEVIATION: 0.0 floor added |
| `0x414DE4` | class-11 item dispatcher (36 cases) | **GAP-VALUE, fixed** |
| `0x410010` | amulet-part combiner | parity |

**F-IT-05 — `0x41DAB0` potion/target-use values — fixed.** The findings came from
the instruction stream (the pseudocode hides the x87 operands behind the `round`
helper `0x43EAae`):

- **subtype 8** is gated by `periodic != 0` and computes
  `periodic = max(0, round(periodic - quality*10))` (`FILD +0x50`, `FSUBR`;
  `0x4501CC` = 10.0). Was `quality*10 - base_armor` with no gate — wrong field
  *and* wrong sign. Now an antidote that still consumes itself when unpoisoned.
- **subtype 7 (oil)** requires `quality >= 10.0` (`0x4501C4`); was `< 0.0`.
- **subtype 13** requires `quality >= 10.0` (`0x4501EC`); was ungated.
- **subtype 6 on a target** adds to the target's existing `+0x0C`
  (`target+0x0C = round(target+0x0C + quality)`); was an overwrite.
- **subtype 11** sets `_DAT_0081D628 = 1` (ground-marker highlight) through the
  new `StatOp::HighlightGround` → `torch_reveal`.
- **subtype 13** on a dead user now runs `FUN_00414ABC` placement
  (`revive_owner_on_free_cell`, the same `revive_cell` helper as the class-11
  resurrection) instead of only clearing the flag.

**F-IT-06 — `0x414DE4` case 30 and case 12 — fixed.** Case 30 now filters on
`Group+0x1E` (`class & 0x80` citizens survive, `class == 0x53` survives) instead
of `Group.loc_id`, so villagers are no longer killed and class-`0x53` groups are
no longer spared. Case 12 now sizes the buff from `dexterity_current` (`+0x56`,
`(int)(+0x54)>>16`) instead of `dexterity_base`.

Verified in the same pass: all 36 implemented `0x414DE4` cases (orb/key
destination constants, the `FUN_004101E4` scatter offsets ±40/-9 and ±20/18,
the doll/magnet/revive/mirror filters, the sword forge field writes, the
amulet table), the 33-row alchemy table `0x450DF4` byte-for-byte, its five
quality formulas (all three constants `0x45011C/24/2C` = 0.01, cap `0x450134` =
15.0) and the `0x41E160` container compaction. `0x41BB70` deviation: the remake
floors the product at 0.0 where the original only caps it at 15.0 (unreachable
with shipped data). `0x41E160` note: the original's container snapshots
(`0x81D404/10/50/00`) are modelled by `HeldItem.source`; its bag snapshot
appends the removed id at the tail, the remake restores the recorded index.

**Verification.** `cargo test` 585
(`antidote_reduces_periodic_damage_only_while_poisoned`,
`oil_below_ten_quality_is_rejected`,
`poison_adds_to_the_existing_target_poison`,
`subtype_eleven_requests_the_ground_highlight`,
`resurrection_potion_requires_quality_ten`,
`c4_skeleton_lamp_spares_citizens_and_class_53`,
`c2_wine_uses_the_current_dexterity`,
`antidote_reduces_periodic_damage`,
`highlight_potion_sets_the_torch_reveal`, plus the updated oil tests);
`cargo check --all-targets` clean; wasm rebuilt; `smoke`, `save_smoke`,
`trade_smoke`, `feedback_smoke`, `combat_smoke` pass.

**E4 (5 rows + 1 cross-referenced COMBAT row): 1 parity, 5 fixed.**

| VA | function | verdict |
|---|---|---|
| `0x410B08` | level-up / stat-raise gate (caps, minima, cost tiers) | **GAP-VALUE, fixed** |
| `0x41AB7C` | total armor (`+0x4E` + slots 3..5 + `+0xC4`) | **GAP-BRANCH, fixed** |
| `0x41C69C` | equipment condition-bonus recalculation | **GAP-MISSING, fixed** |
| `0x41BA08` | identification scroll | parity |
| `0x42F5F8` | lower-panel item detail (`hover_detail`) | **GAP-BRANCH, fixed** |
| `0x41B1A4` | requirement stat value (COMBAT, added here) | **GAP-VALUE, fixed** |

Tables verified byte-for-byte this pass: `STAT_GROWTH_CAPS`/`STAT_GROWTH_MINS`
against `0x45B0AB` (520 bytes, `[cap, min]` per level), `STAT_TIERS` against
`0x45AFB0` (13 rows) and the specialization bytes `0x45AE28` = `[3,6,2,3,5]`.

**F-CH-01 — `0x410B08` sentinel cost — fixed.** The 13th tier is
`(999999, 999999, 999)`; the remake's `stat_cost`/`raise_cost_index` return
`999`, which the character screen would show as a real price once the player
holds 999 spare XP (a maxed stat must show nothing).

**F-CH-02 — `0x410B08` temporary-effect gate — fixed.** `Entity+0xCE != 0` returns 0
(no cost, no level-up). `character_screen::stat_raise_cost` /
`level_up_available` do not check it; the settlement-training caller does.

**F-CH-03 — `0x41C69C` condition-bonus recalculation — fixed.** Clears
`0x81B930` (9 ints) and `0x81CFB8` (9 bytes = 1), walks the 6 equipment, the 5
additional slots and the entity bytes `+0xD2`/`+0xD3`, and for every
**identified** instance with `condition >= 0` applies the record's three
`(stat, op, i16 value)` triples (`active &= op`; a cleared bit replaces the
accumulator, otherwise the value adds). Then stats 2/3/4/5/6 are written to the
derived fields `+0x54/+0x58/+0x5C/+0x5E/+0x62` (HP bonus `<<4`, cap 1600) and
stats 7/8/9 to `+0xC4/+0xC6/+0xC8` with the `+0xD1` add flags — the armor,
damage and attack set bonuses that `0x41AB7C`/`0x41AC40`/`0x41AD44`/`0x41B42C`
consume. 129 shipped `Game.1` instances carry a condition (armor ±4..48 at
records 40..49, damage ±4..48 at 0..9, strength/dex/endurance ±1..6 at 10..39),
so the bonuses are live content.

**F-CH-04 — `0x41AB7C` `+0xC4` armor term — fixed.** `0x41AB7C` adds the
set-bonus word (`+0xC4 == 0` → none, `+0xD1 & 1` clear → replace, else add).
Both remake armor builders stop at base + slots 3..5.

**F-CH-05 — `0x41B1A4` requirement stat map — fixed.** The original maps 1/3 →
charisma, 2 → dexterity, 4 → HP/16, 5 → strength, 6 → endurance, 7 → armor,
8 → `0x41AEA8` damage rating, 9 → `0x41B42C` attack. The remake maps 1/3 → dex,
2 → the venom word `+0x52`, 4 → charisma/16, 5 → HP, 6 → strength, 9 → 0. 66 of
the 70 weapon/armor prototypes carry a requirement on stat 2/5/6, so
`requirement_ok` currently gates on the wrong stat (dexterity requirements read
venom, strength requirements read HP, endurance requirements read strength).

**F-CH-06 — `0x42F5F8` weak-concentration marker — fixed.** Class-9 items with quality
< 10 (subtypes 7/resurrect) or < 6 (subtype 12) append
`", недостаточная концентрация"`; `hover_detail` never prints it. Class 10 has
no switch case in the original (nothing appended); the remake prints
`Неведомый предмет` (left as a readable fallback, documented).

**Naming note.** The original's *derived* stat values are the remake's
`*_base` fields (`+0x54/+0x58/+0x5E/+0x62`); its *stored* values are the
remake's `*_current` (`+0x56/+0x5A/+0x60/+0x64`), which potions, training and
the level-up commands modify. `0x41C69C` recomputes the former from the latter;
`requirement_stat_value` now reads the derived fields.

**Verification.** `cargo test` 596 (`character::effects::tests` — 7 cases for
the triples, replace/add, HP, `state_d2`, unidentified and accumulation;
`requirement_stat_map_reads_the_derived_fields`;
`armor_total_applies_the_c4_set_bonus`; `temp_effect_blocks_raise_and_level_up`;
`hover_detail_marks_weak_potions`; the updated sentinel/raise/trade tests);
`cargo check --all-targets` clean; wasm rebuilt; `smoke`, `save_smoke`,
`trade_smoke`, `feedback_smoke`, `combat_smoke`, `mob_ai_smoke`,
`ranged_smoke` pass.

**Wiring.** `character/effects.rs` is the recalc; `PlayableApp` runs it on game
start, save load, map entry, every equip/deposit/craft, after potion and
class-11 stat operations, after a stat raise, in the 16-frame temporary-effect
pass, and in the role-4 distribution's requirement gate. `EntityStats`/
`PlayerCharacter` carry the three `i16` bonuses + `u8` flags as `serde(skip)`
runtime fields, so the save layout is unchanged (older saves recompute on
load). `CombatStats` gained `armor_bonus`/`bonus_flags` for `armor_total`; the
damage (`0x41AC40`/`0x41AD44`) and attack (`0x41B42C`) terms remain A2 scope
even though the fields are now populated.

---

## 10. Wave A2 — damage chain (audited 2026-11-20)

**12 rows: 4 parity, 3 deviations, 1 platform, 4 fixed.**

| VA | function | verdict |
|---|---|---|
| `0x41B060` | melee hit test (roll vs `0x41B42C`, then `0x41AEA8`) | parity |
| `0x41B42C` | hit/reaction value (dex, skill, distance, `+0xC8`) | **GAP-VALUE, fixed** |
| `0x41AEA8` | weapon damage (`STR*HP`, modifier table, poison, `+0xC6`) | **GAP-BRANCH, fixed** |
| `0x41AC40` | projectile damage (ammo payload × bow rating, `+0xC6`) | **GAP-VALUE, fixed** |
| `0x41AD44` | archer attack value (character screen) | **GAP-BRANCH, fixed** |
| `0x41BF44` | arrow spawn (pool, muzzle, velocity, bow wear) | PARITY-DEVIATION: ammo vs bow payload |
| `0x420F30` | projectile update (8 sub-steps, hits, settlement wake) | parity |
| `0x41C3CC` | melee application (facing dodge, mitigation, wear) | parity |
| `0x41C1E8` | ranged application (dex dodge, mitigation, wear, poison) | parity |
| `0x418480` | corpse loot | PARITY-DEVIATION: no empty marker |
| `0x436214` | Win32 command dispatcher | N/A-PLATFORM |
| `0x438798` | generic rect-table lookup (map click) | PARITY-DEVIATION |

**F-A2-01 — `0x41B42C` inverted reaction formula — fixed.** The original is
`round(100 - (100 - 0.5*dex) / (1.5 + 0.00015*skill))` (`0x45006C` = 0.5,
`0x450074`/`0x45008C`/`0x4500AC` = 100.0, `0x450094`/`0x4500B4` = 0.00015,
`0x45009C`/`0x4500BC` = 1.5) and, for an active bow, multiplies by
`0.7*(1 + distance/bow_range)` (`0x45007C` = 0.7, range = the prototype
`u16@0x459B82 + class*0x40`); it then adds the temp-effect id `+0xCA` while
`+0xCE != 0` and applies the `+0xC8` set bonus (`+0xD1 & 4`). `combat::reaction`
computes `100 - (1.5 + 0.00015*skill) / (100 - 0.5*dex)` — numerator and
denominator swapped — takes no distance and ignores `+0xC8`. New-game hero
(dex 10, melee 100): original 37, remake 100. The object-family branch
(`round(50 + 0.25*dex)`) is correct.

**F-A2-02 — `0x41AEA8`/`0x41AC40` damage set bonus — fixed.** Both damage paths end with
the `+0xC6` term (`0` -> unchanged, `+0xD1 & 2` clear -> replace, else add).
`CombatStats` has carried `damage_bonus`/`bonus_flags` since E4 but
`attack_damage`/`ranged_damage` ignore the term.

**F-A2-03 — `0x41AC40` projectile payload — fixed.** `FUN_0041BF44` copies the **ammo**
instance (`+0xCC`) fields into the projectile: `+0x1E` modifier, `+0x1F`
subtype, `+0x18` poison, plus the **bow's** class rating at `+0x16`. The damage
base is `modifier_table[ammo_modifier]` (when its archetype matches) else
`rating_table[ammo_subtype]`, times the bow rating. `BowShot` is built from
`equipment[1]` (the bow), so `ranged_damage` multiplies by the bow's subtype
rating instead of the arrow's and applies the bow's poison.

**F-A2-04 — `0x41AD44` archer attack display — fixed.** The original shows
`round(rating_table[ammo class]) + +0xC6` as «Атака» while a bow is active; the
remake's character screen shows 0 for `raw_0x12 == 1`.

**F-A2-05 — `0x418480` empty marker.** With no loot at all the original still
creates an empty ground record on a `rand()%101 <= 0x1E` roll (a corpse marker
with no money/items); the remake always returns an empty plan.

**Verified in the same pass.** The projectile geometry (`0x41BF44`: pool 100,
`y-30` aim, normalized `|d|*0.25 + 0.001`, muzzle `0x45ACD0`, sprite
`0xB8+2f`, bow wear `-0.1`), the update pass (`0x420F30`: 8 sub-steps, `0xC000`
walls, occupancy hits, `+0x4B2 |= 0x15`, group wake, `0x41B658` power), the
mitigation `round(16*(1 + (d*0.3 + max(0, d*0.7 - armor))/endurance))`, the
facing dodge table `0x450DB4` + `dex*0.5`, the ranged dodge `dex*0.5`, the
wear scales `0.001`/`0.0001`, and the corpse-loot money/22-slot/stinger/generated
rules.

**Fixes (2026-11-20).** `combat::reaction_of(ReactionInput)` is the pure
`0x41B42C` formula (`combat::reaction(stats, distance, bow_range)`,
`combat::bow_range`, `roll_hit(attacker, bow_range, rng)`); new-game hero 37,
ant 59, archery 56 -> 12 over the range. `attack_damage` and `ranged_damage`
apply the `+0xC6` term (`+0xD1 & 2`); `CombatStats` now carries `ammo` (the
`+0xCC` stack), `damage_bonus`, `attack_bonus` and the temporary-effect fields,
and `bow_shot` takes the bow rating plus the **ammo** modifier/subtype/poison
(`ranged_damage` also mirrors the original's `modifier < 1` rule). The
character screen shows `0x41AD44` (ammo rating + `+0xC6`) for an active bow and
`requirement_stat_value` stat 9 is the real `0x41B42C(entity, 1)` value. The
debug ranged staging clamps the staged HP to the `0x41C69C` 1600 cap.

**Verification.** `cargo test` 599 (`reactions_match_the_verified_formulas`
updated to 37/59, `archery_reaction_scales_with_distance`,
`attack_damage_applies_the_c6_bonus`, `ranged_damage_applies_the_c6_bonus`,
updated bow/projectile tests); `cargo check --all-targets` clean; wasm rebuilt;
`smoke`, `save_smoke`, `trade_smoke`, `feedback_smoke`, `combat_smoke`,
`mob_ai_smoke`, `ranged_smoke` pass.

---

## 11. Wave A2b — AI remainder (audited 2026-11-20)

**26 rows: 18 parity, 7 deviations, 1 fixed.**

| VA | function | verdict |
|---|---|---|
| `0x410450` | human idle/route-pending helper | parity |
| `0x410728` | object-family idle helper | parity |
| `0x410FFC` | first free neighbour (8 directions) | parity |
| `0x411758` | ordered target at a cell | parity |
| `0x411FD0` | human arrival handler (row 5) | PARITY-DEVIATION: folded into the segment commit |
| `0x4123BC` | human action machine (rows 0..9) | parity |
| `0x413A84` | random wander target | parity |
| `0x413DE4` | frame advance + anchors | parity |
| `0x414A34` | idle | parity |
| `0x415C40` | object arrival handler | PARITY-DEVIATION: folded |
| `0x415FC8` | object action machine | parity |
| `0x4168A4` | citizen AI (waypoints/wander) | PARITY-DEVIATION: `Q`/`R` 75% skip roll missing |
| `0x4170E0` | random kit selector | parity |
| `0x417190` | deterministic kit selector | parity |
| `0x41F048` | follower arrival handler | PARITY-DEVIATION: follower timer (F-AI-13) |
| `0x421964` | settlement activation snap | PARITY-DEVIATION: all-20 pass (Wave C) |
| `0x428DB0` | HUD button state setter | parity |
| `0x42BC60` | actor name builder | parity |
| `0x42C730` | portrait selector | parity |
| `0x42CA44` | HUD button strip | parity |
| `0x430958` | actor hover panel | parity |
| `0x4353B0` | camera clamp/follow | parity |
| `0x435870` | location dead-actor cleanup | **GAP-BRANCH, fixed** |
| `0x438104` | party-selection removal | parity |
| `0x43CAB5` | clipped sprite blit | PARITY-DEVIATION: RGBA8 |
| `0x43D971` | intensity/palette remap blit | PARITY-DEVIATION: RGBA lighting |

**F-AI-15 — `0x435870` dead-actor cleanup at location setup — fixed.** Called only from
the command dispatcher `0x436214`: for every dead actor (`flags & 0x80`) of the
location it runs `FUN_00422520` (release every carried item instance,
`class 0xFF` - the same routine `0x16B` uses) and clears the settlement
waypoint owner slots (`+0x3D2..+0x3DA`) pointing at it. The remake only calls
`0x422520` from `remove_world_entity`, so dead NPC gear stays allocated and a
dead specialist can keep a waypoint slot until `0x16B`/re-assignment.

**Notes.** The arrival handlers are folded into `begin_*_segment` (the remake
clears the path, stops on a foreign occupant and re-routes; contact attack when
the occupant is the ordered target), which reproduces the row-5 decisions
without a separate pass. `0x4168A4`'s `Group+0x1D == 0` + archetype `Q`/`R`
`rand()%100 < 0x4B` skip is the only citizen-AI branch the remake lacks. The
settlement activation snap (`0x421964`) has no counterpart because `0x41D048`
runs for all 20 settlements (Wave C). The 16-bit blits (`0x43CAB5`,
`0x43D971`) are substituted by the RGBA8 compositor and the lighting path.

**Fix (2026-11-20).** `app/population.rs::cleanup_dead_at_location` walks
the location's groups, releases every dead actor's item instances
(equipment/ammo/inventory/additional, `class 0xFF`) and zeroes the settlement
waypoint owners (`+0x3D2..+0x3DA`) of the settlement's own group that point at
them. `ensure_population` runs it on every map entry (the travel path drops the
cache first), so each entry repeats the original's setup cleanup.

**Verification.** `cargo test` 600
(`map_entry_releases_dead_actor_items_and_waypoints`: instance `0xFF` after the
sweep, waypoint owner 0 after the map entry); `cargo check --all-targets`
clean; wasm rebuilt; `smoke`, `save_smoke`, `trade_smoke`, `feedback_smoke`,
`combat_smoke`, `mob_ai_smoke`, `ranged_smoke` pass.

---

## 12. Wave M — movement (audited 2026-11-20)

**14 rows: 10 parity, 4 deviations.**

| VA | function | verdict |
|---|---|---|
| `0x41165C` | neighbour-direction probe | parity |
| `0x414078` | per-frame sprite/anchor resolution | parity |
| `0x4142D4` | walk command (dest + route + walk row) | parity |
| `0x41435C` | actor logic step (render fields + action boot) | parity |
| `0x414ABC` | revive placement | PARITY-DEVIATION: rect/ring fallback vs leader box |
| `0x438090` | one-step facing turn | PARITY-DEVIATION: facing snaps |
| `0x438640` | facing direction probe | PARITY-DEVIATION: sector formula |
| `0x43881C` | grid distance (Chebyshev + 1) | parity |
| `0x43897C` | direction from cell to cell | parity |
| `0x438A78` | actor projection | parity |
| `0x438AB4` | inverse projection (screen -> cell) | parity |
| `0x438ED8` | half-res inverse projection | PARITY-DEVIATION: camera works in pixels |
| `0x43D922` | direction step (parity tables) | parity |
| `0x43DA1F` | route builder (A*, direction queue) | parity |

**Deviation notes.** `0x414ABC` revives on the current cell, else a free
neighbour, else a random cell in the leader's `+-2*radius` box; the remake scans
the actor's authored rectangle and a ring instead of the leader box. `0x438090`
turns the facing one step per frame toward the target (then re-resolves the
sprite); the remake assigns the facing directly, so only the intermediate
frames differ. `0x438640` probes the lattice step (`0x43D922`) in the order
6/7/5/2/3 with a fallback of 1 for non-adjacent cells; the remake uses the
sector formula (`0x43897C`) for the same facing updates. `0x438ED8` is the
160x80 inverse used by the camera and the world-map draw; the remake works in
local pixels and map rects.

**Verification.** Audit-only pass (no code changed): `python tools/parity_index.py
--check` clean; test/smoke state unchanged.

---

## 13. Wave R — rendering (audited 2026-11-20)

**30 rows: 21 parity, 6 deviations, 3 platform.**

| VA | function | verdict |
|---|---|---|
| `0x424ED0` | actor overlay shadow masks | parity |
| `0x42532C` | world draw pass (terrain/actors/markers/rings) | parity |
| `0x426384` | shadow/depth pass (span buffers) | PARITY-DEVIATION: RGBA halving |
| `0x427698` | text queue push (window) | parity |
| `0x42770C` | text queue push (world) | parity |
| `0x427780` | string queue push | parity |
| `0x427A3C` | 16-bit colour brighten | PARITY-DEVIATION: RGBA blend |
| `0x427D94` | indexed RLE sprite decoder | parity |
| `0x428390` | world-map draw | parity |
| `0x42863C` | frame begin (viewport + ambient light) | parity |
| `0x428DDC` | HUD button state sync | parity |
| `0x429744` | DirectDraw/DirectSound surface release | N/A-PLATFORM |
| `0x4297D4` | primary surface release | N/A-PLATFORM |
| `0x4299FC` | DirectDraw initialisation | N/A-PLATFORM |
| `0x42A470` | cursor blit + frame flush | parity |
| `0x42BAE4` | equip attempt feedback | parity |
| `0x42BD8C` | wrapped text draw | parity |
| `0x42CBD0` | interface mode renderer | parity |
| `0x42E520` | member panel refresh | parity |
| `0x42E8EC` | party panel loop | parity |
| `0x42E9CC` | lower-panel message setup | parity |
| `0x42EA6C` | New-Hero screen | parity |
| `0x4304F4` | HUD text push | parity |
| `0x438CC8` | ground-marker sprite resolver | parity |
| `0x43C1DC` | clipped 16-bit sprite blit | PARITY-DEVIATION: RGBA8 |
| `0x43C557` | masked 16-bit sprite blit | PARITY-DEVIATION: RGBA8 |
| `0x43CEAA` | 16-bit sprite blit | PARITY-DEVIATION: RGBA8 |
| `0x43D2FE` | colour-key cursor decoder/blit | PARITY-DEVIATION: RGBA8 |
| `0x43D64D` | RLE span expander | parity |
| `0x43D6A5` | rectangle fill/clip | parity |

**Deviation notes.** The DirectDraw surface objects (`0x429744`/`0x4297D4`/
`0x4299FC`) have no counterpart in the browser shell (RGBA framebuffer +
WebAudio). The 16-bit blits (`0x43C1DC`/`0x43C557`/`0x43CEAA`/`0x43D2FE`) and
the colour brighten (`0x427A3C`) are substituted by the RGBA8 compositor; the
shadow pass (`0x426384`) halves RGBA channels instead of `(c & 0xF7DF) >> 1`
and has no run budget (projectile shadows unported). All geometry, clipping,
sprite tables and draw order match.

**Verification.** Audit-only pass (no code changed): `python tools/parity_index.py
--check` clean; test/smoke state unchanged.

---

## 14. Waves I and L — input and loading (audited 2026-11-20)

**19 rows: 12 parity, 6 deviations, 1 platform.**

| VA | function | verdict |
|---|---|---|
| `0x42033C` | cursor self-use of a held class-9 item | parity |
| `0x42260C` | main mouse handler (UI codes + world click) | PARITY-DEVIATION: right-click actor order |
| `0x4242BC` | quest-log toggle | parity |
| `0x424330` | character-screen toggle | parity |
| `0x424E0C` | cursor id from item class | parity |
| `0x428740` | cursor frame selector | parity |
| `0x428C00` | mouse motion accumulate/clamp | parity |
| `0x435B0C` | keyboard (VK) dispatch | parity |
| `0x4381C8` | interface click hit test | parity |
| `0x4246C0` | save-game loader | PARITY-DEVIATION: native snapshot/legacy parser |
| `0x424B3C` | `KONUNG.SAV` name table | parity |
| `0x42781C` | resolution/viewport setup | parity |
| `0x42C1F4` | table-file loader A | PARITY-DEVIATION: pack bytes |
| `0x42C5A0` | table-file loader B | PARITY-DEVIATION: pack bytes |
| `0x43A75C` | file read helper | PARITY-DEVIATION: browser storage |
| `0x43A870` | `Game.N` content loader | parity |
| `0x43ABFC` | table-file loader C | PARITY-DEVIATION: pack bytes |
| `0x43ADF0` | map loader (mode 0/1) | parity |
| `0x43BFE4` | frame timer | N/A-PLATFORM |

**Deviation notes.** The mouse handler's one gap is the documented one: a
right-click on a non-talkable actor falls through to a move instead of issuing
the party attack order. All four loaders (`0x4246C0` save, `0x42C1F4`,
`0x42C5A0`, `0x43A75C`, `0x43ABFC`) are I/O substitutions: the remake restores
the same state from the pack (`bundle.bin`) and the native `OPKS`/`KONUNG.SAn`
parsers instead of Win32 file handles. `0x43BFE4` is the `timeGetTime` clock
replaced by the animation-frame delta.

**Verification.** Audit-only pass (no code changed): `python tools/parity_index.py
--check` clean.

---

## 15. Wave S — audio (audited 2026-11-20)

**12 rows: 2 parity, 10 platform.**

| VA | function | verdict |
|---|---|---|
| `0x429540` | MCI pause | N/A-PLATFORM |
| `0x429570` | MCI open | N/A-PLATFORM |
| `0x429630` | MCI play | N/A-PLATFORM |
| `0x4296B0` | MCI restart | N/A-PLATFORM |
| `0x429714` | MCI stop | N/A-PLATFORM |
| `0x429820` | DirectSound buffer release loop | N/A-PLATFORM |
| `0x42987C` | sound teardown | N/A-PLATFORM |
| `0x429908` | sample memory release | N/A-PLATFORM |
| `0x429948` | sample memory allocation | N/A-PLATFORM |
| `0x429C0C` | DirectSound initialisation (3 buffers) | N/A-PLATFORM |
| `0x42A13C` | sound play dispatcher (id/volume/pan/rate) | parity |
| `0x4392B4` | sound-system initialisation (`SOUNDS.RES`) | parity |

**Notes.** The data path is ported: `sound.rs` decodes `SOUNDS.RES` (8-byte
header + 1000 `{u32 off,u32 size}` entries, 391 clips) and the play dispatcher's
semantics live in `SoundEvent` + `sound/position.rs` (pan `round(dy*62.5)`,
volume `dist*-10000/1600` cut at `-4000`, the `id < 0x20` / `rand()%3` rate
rule), exposed through `sound_pan/volume/rate` and applied by the WebAudio
shell; `sound/events.rs` holds the 22 verified ids. Everything that touches
Win32 (MCI music, DirectSound buffers, the global sample heap) is
platform-substituted by the browser audio graph.

**Verification.** Audit-only pass (no code changed): `python tools/parity_index.py
--check` clean.

---

## 16. Wave X — misc helpers (audited 2026-11-20)

**41 rows: 34 parity, 6 deviations, 1 platform.**

| VA | function | verdict |
|---|---|---|
| `0x410E10` | ground-marker lookup by cell | parity |
| `0x411708` | selection membership | parity |
| `0x4118DC` | load/map-entry occupancy rebuild | parity |
| `0x4138C8` | ground drop | parity |
| `0x413DC4` | empty stub (`return 0`) | parity |
| `0x421458` | party engage | parity |
| `0x421554` | battle/peace stance toggle | parity |
| `0x4216DC` | active weapon slot cycle | parity |
| `0x42246C` | party halt | parity |
| `0x424520` | effect/projectile animation tick | parity |
| `0x424D8C` | party-cell selection | parity |
| `0x427A94` | indexed sprite decoder variant | parity |
| `0x427E9C` | human overlay compositor | parity |
| `0x428AD8` | effect queue compaction | parity |
| `0x428C94` | camera centering/clamp | parity |
| `0x428E98` | cursor reset | parity |
| `0x428F94` | member-panel return | parity |
| `0x428FE0` | ambient light | parity |
| `0x42ADE4` | movie player trigger | parity |
| `0x42B098` | AVI/DirectShow teardown | N/A-PLATFORM |
| `0x42BBC4` | `Уров:N` text fragment | parity |
| `0x42BD28` | string copy/truncate helper | parity |
| `0x42C384` | settings load (`KONUNG.CFG`) | PARITY-DEVIATION: localStorage |
| `0x42F578` | New-Hero control description | parity |
| `0x43060C` | centered window/message draw | parity |
| `0x4357A0` | ground-marker animation pass | parity |
| `0x438884` | free item-instance allocator | parity |
| `0x438900` | button run-table hit test | parity |
| `0x438DC4` | height/depth compare helper | parity |
| `0x438E24` | projection depth compare | parity |
| `0x4390D0` | settlement lookup by location | parity |
| `0x439130` | RNG bounded draw | parity |
| `0x4391CC` | RNG seed reset | parity |
| `0x43BEBC` | voice-handle table rotate | parity |
| `0x43BF5C` | fatal error/exit path | parity |
| `0x43C040` | shadow mask adder | parity |
| `0x43C9A2` | 16-bit span copy | PARITY-DEVIATION: RGBA8 |
| `0x43CC49` | 16-bit span copy (offset) | PARITY-DEVIATION: RGBA8 |
| `0x43D247` | shadow darken | PARITY-DEVIATION: RGBA halving |
| `0x43D7BF` | 16-bit span fill | PARITY-DEVIATION: RGBA8 |
| `0x43D80F` | 16-bit span copy (variant) | PARITY-DEVIATION: RGBA8 |

**Notes.** The play-side commands (`0x421458` engage, `0x421554` stance,
`0x4216DC` weapon slot, `0x42246C` halt, `0x424D8C` selection), the cursor reset
(`0x428E98`), the ground items (`0x410E10`, `0x4138C8`, `0x4357A0`), the
compositor (`0x427E9C`), the RNG (`0x439130`/`0x4391CC`), the instance allocator
(`0x438884`) and the HUD run-table test (`0x438900`) all match their ports. The
only data-path deviation is `0x42C384` (the 28-byte `KONUNG.CFG` read is
`localStorage` in the browser). The five span/blit helpers are the RGBA8
substitution already recorded for RENDER; `0x42B098` is DirectShow teardown.

**Verification.** Audit-only pass (no code changed): `python tools/parity_index.py
--check` clean.

---

## 17. Wave P - deviation triage (2026-11-20)

The 44 `PARITY-DEVIATION` rows were split by cause. **22 were not behavioural
gaps** and were re-audited or reclassified; **22 real gaps remain**, and all of
them are scheduled for a code fix (sections 18-20).

### 17.1 Re-audited to PARITY (5)

| VA | why it is not a deviation |
|---|---|
| `0x411FD0` | human arrival: the decision is made at the walk-segment commit (contact attack on the ordered target, otherwise stop + clear the queue + idle) - structural fold |
| `0x415C40` | object arrival: the same fold through `begin_actor_segment` |
| `0x41F048` | follower arrival: the same decision lives in `app/party.rs::follow_leader` |
| `0x41BF44` | arrow spawn: the code already carries the **ammo** modifier/subtype/poison plus the **bow** rating (`combat::bow_shot` -> `Projectile.bow`); the old deviation text was stale |
| `0x438798` | rectangle-table scan: the remake keeps the same per-resolution rect data and tests it directly |

### 17.2 Reclassified to N/A-PLATFORM (17)

Win32 file I/O: `0x4246C0`, `0x42C1F4`, `0x42C5A0`, `0x43A75C`, `0x43ABFC`
(loaders), `0x42C384` (`KONUNG.CFG` -> `localStorage`).

16-bit pixel store: `0x43C9A2`, `0x43CC49`, `0x43D7BF`, `0x43D80F`, `0x43C1DC`,
`0x43C557`, `0x43CEAA`, `0x43D2FE`, `0x43CAB5`, `0x43D971` (span/sprite blits
generated from the same rects and masks).

Engine geometry: `0x438ED8` (160x80 inverse projection; the camera works in
local pixels).

These are substitutions of a platform primitive, not logic: the browser has no
Win32 file handles and composites in RGBA8. `0x427A3C` (565 brighten) and
`0x43D247` (565 shadow darken) stay deviations for now because they **can** be
made bit-exact with a 565 round-trip (section 19).

### 17.3 Remaining 22 deviations (the close-out list)

| domain | VA | gap |
|---|---|---|
| AI | `0x4168A4` | citizen 75 % skip roll + `+0x1D == 0` / `Q`/`R` gate |
| AI | `0x421964` | current-settlement activation snap (`_81D6D8` + leader rect) |
| AI | `0x421B50` | one fully interleaved group pass (kept as `wontfix`) |
| AI | `0x424410` | give-held always front-inserts (no ammo merge) |
| COMBAT | `0x418480` | 30 % empty corpse marker |
| COMBAT | `0x41B0C4` | class-10 item weight must be 0 |
| INPUT | `0x42260C` | right-click non-talkable actor -> party attack order |
| MISC | `0x4105A0` | equipment refresh charges one AI action |
| MISC | `0x4182D4` | condition-record requirement gate |
| MISC | `0x41BB70` | alchemy floor 0.0 + held class gate |
| MISC | `0x43D247` | 565 shadow darken |
| MOVEMENT | `0x414ABC` | revive leader-box fallback |
| MOVEMENT | `0x438090` | one-step-per-frame facing turn |
| MOVEMENT | `0x438640` | facing probe step order + fallback |
| QUEST | `0x430430` | `_81D614` filled/tiled dialogue background |
| QUEST | `0x432120` | signed target key (out-of-bounds word) |
| RENDER | `0x426384` | shadow run budget + projectile shadows + 565 darken |
| RENDER | `0x427A3C` | 565 brighten |
| SETTLEMENT | `0x4110EC` | discard window (a) + pickup ammo merge (b) |
| SETTLEMENT | `0x4172AC` | prop rebuild uses the game RNG + shared record refcount |
| TRADE | `0x417010` | ground-container / discard window |
| TRADE | `0x41EAD8` | held-item sentinel branch |

**Verification.** Ledger-only pass: `python tools/parity_index.py --check` clean.

---

## 18. Wave D1 - small deviation fixes (2026-11-20)

**12 deviations closed, 1 structural part deferred.** Ledger: 188 -> 200 parity,
22 -> 10 deviations.

| VA | original rule | fix |
|---|---|---|
| `0x41B0C4` | class-10 weight is **0** | `item_weight` maps class 10 to 0 |
| `0x418480` | empty body still creates a marker on `rand()%101 <= 0x1E` | `LootPlan::empty_marker` + `spawn_corpse_loot` |
| `0x41BB70` | no lower quality clamp; only the target must be class 9 | dropped `.max(0.0)`, held class gate removed |
| `0x4182D4` | condition-record gate + class split | `requirement_ok` reads `0x458530` |
| `0x4105A0` | refresh from the wake path, charges one action | moved into the `FUN_00412E00`/`FUN_004168A4` wake branch |
| `0x424410` | give-held always front-inserts | ammo merge removed |
| `0x4168A4` | `Q`/`R` skip 75 %, others 50 %, both `rand()%100` | two thresholds replace `rng % 2` |
| `0x4110EC` | pickup never stacks | merge removed (discard window still open, D3) |
| `0x41EAD8` | id-vs-`_81D670` occupancy test | re-audited: `id != 0` is exact in the remake model |
| `0x414ABC` | neighbour probe then leader/rect-A box, `rand` start, 100 probes | `revive_cell` rewritten |
| `0x438640` | steps `6,7,5,2,3`, fallback 1 | `coords::face_direction` |
| `0x430430` | `_81D614` tiles or leaves the frame | `dialog_background_tiled`, VK `T` |
| `0x432120` | signed key, unguarded `0x458A06 + key*0x40` | 256-word table captured from `konung.res` |

**Tests.** `coords::tests::face_direction_uses_the_original_step_order`,
`combat::tests::empty_corpse_marker_follows_the_low_roll`,
`app::tests::inventory::class_ten_items_weigh_nothing`,
`app::tests::combat_ai::woken_actor_refreshes_gear_and_charges_one_action`,
`app::tests::class11::revive_cell_falls_back_to_the_leader_box`,
`app::dialogue_ops::tests::requirement_timer_reads_the_signed_key_address`.
`cargo test` 606 pass; wasm rebuilt; combat, mob-AI, ranged, trade, save, map,
feedback and shadow smokes pass.

**Behavior notes now in `AGENTS.md`.** The equipment refresh is the wake frame
(one charged action) instead of a per-tick batch; pickup and give-held always
front-insert; the revive box and the signed target timer are exact; VK `T`
toggles the dialogue stone.

---

## 19. Wave D2 - input, settlement props, shadows (2026-11-20)

**4 deviations closed, 1 reclassified as platform.** Ledger: 200 -> 204 parity,
10 -> 5 deviations.

| VA | finding | fix |
|---|---|---|
| `0x42260C` | the attack order already existed; the dialogue gate was missing the Shift test (`_81D624`) | `world_interact` requires `!shift_down` to talk |
| `0x4172AC` | prop rebuild used a stable hash | game RNG, `rand() % (max-min+1)`, no draw when a dimension is 1 |
| `0x426384` | no prop shadows, no run budget | `Res::add_prop_shadow`, 11998-run budget, exact darken |
| `0x43D247` | RGBA halving != 16-bit halving | recover 555 -> 565 -> `(c & 0xF7DF) >> 1` -> expand |
| `0x427A3C` | **role was wrong**: it is the 555->565 palette-table conversion (`0x43A31C` at 16bpp), not a light blend | `N/A-PLATFORM` |

### 19.1 Shadows

`FUN_00426384` walks the current-location **groups** (body mask + one mask per
equipped overlay) and then the **prop pool**: a prop's frame record at
`+0x80 + frame*8` holds `{shadow_offset, sprite_offset}`, its anchors at
`+0xC0 + frame*8` hold `{shadow_gx, shadow_gy, sprite_gx, sprite_gy}`, and the
payload is the object block from `+0x100`. A prop casts a shadow only when
`+0x08 < 2` (fewer than two flame parts) and the offset is not `-1`. The pass
never walks the projectile pool, so the old "projectile shadows not ported"
note was describing a feature that does not exist.

`darken` now reproduces the 16-bit arithmetic exactly: the framebuffer holds the
`*255/31` expansion of the 555 palette, so the palette word is recovered,
converted to 565 like the original's 16bpp path, halved with
`(c & 0xF7DF) >> 1` and expanded back. For every 555 colour that is
`(r5 >> 1, g5, b5 >> 1)`, which the plain `c >> 1` did not match at the top of
each channel. The frame's run pool is `0x5076D8 = 0x2EDE` runs.

### 19.2 Settlement props

The start frame and the FLAMES phases now come from the game RNG in the
original order and with the original `+1` span. The shared refcount `0x81B954`
and the hide list `0x81C194` are the original's manual sprite-cache unload
(`0x428AD8` pops the list, reloads the object and clears the hidden bit); the
remake decodes props on demand and has no such cache, so there is nothing to
unload and the visual result is unchanged.

### 19.3 Remaining 5 deviations

| domain | VA | gap |
|---|---|---|
| AI | `0x421964` | current-settlement activation snap (`_81D6D8` + leader rect) |
| AI | `0x421B50` | one fully interleaved group pass (`wontfix`) |
| MOVEMENT | `0x438090` | one-step-per-frame facing turn |
| SETTLEMENT | `0x4110EC` | discard window when the bag/weight is full |
| TRADE | `0x417010` | ground-container / discard window |

The last four (all but `0x421B50`) are Wave D3.

**Verification.** `cargo test` 610; wasm rebuilt; combat, mob-AI, ranged, trade,
save, map, frontend, feedback, shadow and lighting smokes pass. The lighting
smoke now compares sunrise (frame 18000) with night (12000): frame 0 is the
neutral phase, and on the dark start map the night delta clamps at 0, so it
cannot serve as the "day" reference.

---

## 20. Wave D3 - facing turn, ground window (2026-11-20)

**3 deviations closed; 1 kept with a new finding.** Ledger: 204 -> 207 parity,
5 -> 2 deviations.

| VA | finding | fix |
|---|---|---|
| `0x438090` | facing snapped to the target | `coords::turn_toward` + `ActorMotion::turning`; one 45-degree step per logic frame, cell committed only when `facing == queue head` |
| `0x417010` | no ground-container window | `start_ground_trade` reuses the trade state with the pile as the partner side; confirm writes the stock back and finalizes an emptied pile |
| `0x4110EC` | a full bag/weight silently left the remainder | pickup now opens that window on the pile (the refusal message is set) |
| `0x421964` | current-settlement activation snap | **kept**: the rectangle table `0x7891B4` is runtime-initialised (see below) |

### 20.1 The facing turn

`FUN_00438090` rotates one step per frame (`diff = (dir - facing) & 7`,
`diff < 5` clockwise, else counter-clockwise) and `FUN_004123BC` only consumes
the queue head once the facing matches. The remake now keeps `walking` set
while `turning` so the movement tick keeps servicing the actor, turns in
`advance_motion_walk`/`advance_motion_walk_cycle` and in `update_population`,
and the protagonist turns one step per logic frame before its first step. The
citizen station/home facing also turns instead of snapping.

### 20.2 The ground-container window

`FUN_00417010` opens the standard exchange with the ground pile as the partner
(no NPC, no partner money, no settlement multiplier). The remake reuses
`TradeState`/`trade_screen`: rows 0/1 are the bag/offer, rows 2/3 the pile, and
confirm writes `outcome.partner_inventory` back into the `WorldObject`, removing
and finalizing an emptied pile. Cancel returns the player's offers unchanged.
The pickup path opens it whenever a weight or bag refusal leaves items behind.

### 20.3 `0x421964` was misclassified - it is the transition activation

The earlier note called this a "current-settlement activation snap" reading an
unrecoverable table at `0x7891B4`. That was wrong on both counts. `0x43A95A`
in `FUN_0043A870` reads **`0x109A` = 250 x `0x11` bytes from `Game.N`** into
`0x7891B4`; the block is `Game.N +0x29600`, the gap between the WorldObject
table (`+0x21900`) and the Entity table (`+0x2A69A`) - the transition table that
`edges.rs` already decodes (`TRANS_OFF`/`TRANS_COUNT`/`TRANS_REC`). There was no
missing data.

`FUN_00421964` is the **walk-on transition activation**: for the leader (or a
unit on the leader's cell) it finds the transition record whose `src (+0x03)` is
the current location and whose trigger rectangle (`+0x09/+0x0B`-`+0x0D/+0x0F`)
contains the cell, sets the pending destination `_81D6D8 = -dst (+0x04)` and
copies the arrival `+0x05/+0x07` and the `+0x02` facing into the leader group.
Its guard suppresses the record whose table slot equals the current location's
settlement index - the door the party arrived through - while
`Settlement+0x3E != 3` (all shipped states are 0).

The remake already detected the edge (`update_player`: `from`, trigger rect,
lowest `rec`, `travel_to_map` with `+05/+07`), so only two details were missing:
the **arrival facing** (it passed the player's own facing instead of `+0x02`;
the world-map path already used `meta & 7`) and the **arrival-door guard**.
Both are fixed and covered by
`app::tests::movement::the_current_settlement_suppresses_its_arrival_door`.

**Verification.** `cargo test` 615; wasm rebuilt; combat, mob-AI, ranged, trade,
save, map, frontend, feedback, shadow and lighting smokes pass.

---

## 8. Next waves

| wave | system | count | first action |
|---|---|---:|---|
| A2 | damage chain + AI remainder | 38 | **complete**: damage chain 12 (4 parity, 3 deviations, 1 platform, 4 fixed) + AI remainder 26 (18 parity, 7 deviations, 1 fixed F-AI-15) |
| B | trading | **complete** | 13 audited: 9 parity, 2 deviations, 2 fixed; **F-TR-02 fixed** (`0x41079C` XP/auto-spend) |
| C | settlement economics | 17 | `0x41D048` first (it is the pass, not a death handler), then `0x416670`, `0x4175FC`, `0x4179C8`, `0x417F84` |
| D | quest opcodes | **complete** | 66/66 audited: 51 parity, 2 deviations, 13 fixed, 0 open |
| E | items/character | — | **E4 complete** (23 rows: 8 parity, 5 deviations, 10 fixed, 0 open); next A2 damage chain |
| M | movement | **complete** | 14 rows: 10 parity, 4 deviations (revive fallback, facing turn/probe, half-res inverse) |
| F | input/render/save/audio/misc | **complete** | I/L 19, S 12, X 41 — every ledger row is audited |
| P | deviation triage | **complete** | 44 -> 22: 5 re-audited parity, 17 platform; next D1 (13 small), D2 (4), D3 (3) |
| D1 | small deviation fixes | **complete** | 13 rows (12 closed + 1 re-audited): item weight, loot marker, alchemy, requirement gate, wake refresh, give-held, citizen skip, timer key, revive box, facing probe, dialog toggle; remaining: 0x4110EC discard window |
| D2 | input/props/shadows | **complete** | 5 rows: Shift dialogue gate, prop RNG, prop shadows + run budget, exact 565 darken; 0x427A3C reclassified as the 555->565 palette conversion |
| D3 | facing + ground window | **complete** | 4 rows: turn gate, ground-container window, pickup refusal opens it; 0x421964 re-audited to PARITY (transition activation, arrival-door guard + +0x02 facing) |

---

## Appendix A — AI rows audited in this pass

| VA | verdict | summary |
|---|---|---|
| `0x41F3C8` | GAP-VALUE | random 12-slot formation table vs fixed 8-ring |
| `0x41130C` | GAP-VALUE | 4 px/sample DDA vs 1 px |
| `0x410E84` | GAP-VALUE | `equipment[1]` bow gate vs `equipment[0]` |
| `0x4116CC` | GAP-MISSING | 3-actions/frame budget + round-robin cursor |
| `0x41EB48` | GAP-STATE | gate/group/engage-bit/state writes |
| `0x412E00` | GAP-BRANCH | recall random cell + engaged chase; 0x51–0x53 |
| `0x41ECC8` | GAP-BRANCH | per-cycle LOS re-probe in the ranged branch |
| `0x41EFD8` | GAP-ORDER | armed→retaliate→follow serialisation |
| `0x41E2D4` | GAP-STATE | clears bit 4 on re-acquire |
| `0x41FF68` | GAP-ORDER | member-order vs enemy-order shared target |
| `0x41FB64` | GAP-STATE | caller-dependent search box |
| `0x4101E4` | GAP-MISSING | approach offset (`rand()%4` quadrant + slide) absent |
| `0x421B50` | GAP-ORDER | single interleaved pass vs five staged passes |
| `0x412C30` | PARITY | group relation pass matches |
| `0x420190` | PARITY | nearest-unengaged matches |
| `0x420098` | PARITY | plain nearest matches |

---

## Live UI parity (2026-11-21)

The original `konung.res` was run on this machine (scratch install with a
`KONUNG\Video` junction and `KONUNG.CFG`; Esc skips the Indeo intro) and used as
a pixel oracle against the wasm frontend and character-sheet renders. Findings
and fixes:

* **New Hero screen** — hero portrait pairs are **(selected, normal)**
  (`#3/#4`, `#5/#6`, `#7/#8`); numeric values draw top-left at the
  `NEWH_DRAW` rect; the paper-doll preview uses the static 12-entry default
  instance table written by `FUN_00436214` case 3 to `0x659B0C`, not a zeroed
  table.
* **Character sheet (`FUN_0042CBD0` case 3)** — tiles `INTR[5]` over the
  viewport before `INTR[2]`; draws the AUTO button (`INTR[169/170]`,
  `Entity+0x03 & 2`) at the `0x465B34` record-5 anchor and **not** in the world
  HUD; «Удар» is `FUN_0041AEA8(entity, 0)` (or `FUN_0041AD44` for a live bow),
  not the raw item rating; name/class/stats use the fonts' native cyan pixels;
  the lower panel keeps the default quest-log text.
* **Colour** — all RGB555 art is expanded through the original 16-bit **565**
  path (green 6-bit), making R/B bit-exact and G exact against live
  screenshots.

After the fixes the live diff is 0 except the dynamic paper-doll pose/cursor:
New Hero portraits/title/description 0; character-sheet stats/party/AUTO/
free-XP 0, status text 4 px.

---

## 21. Wave E1 - selection rings, draw order, ambient audit (2026-11-22)

Live oracle: `konung.res` started directly from a complete install (the CD
launcher's registry check is bypassed), Esc skipped the intro, `new-game --hero
0 --class warrior` -> Dragomir at loc 4 cell (72,34), matching the remake's boot.
Screenshots at phases 0/6000/12000/18000 x shadows/day-night/light-effects
(`tools/oracle_capture.py`, `tools/oracle_frames.mjs`, `tools/oracle_diff.py`).

**Selection rings (`0x42532C`).** The pass loops `0x81A814` and draws 56x32
INTR ellipses at `viewport + Entity+0xE8/+0xF0 - (0x1D,0x10)`, base palette,
with the exact 8-case walk bob. Health tiers from `Entity+0x5C`:
`< 401` INTR[172] red `(238,41,24)`, `< 801` INTR[126] yellow `(246,230,41)`,
else 178 green `(0,255,0)`; the earlier "cyan/blue" note was a BGR read. Ported
in `app/world_draw.rs` (+ `IntrRole::SelectionRing`), checked by
`app::tests::selection` and `tools/ring_smoke.mjs` (tiers, position, deselect,
unshaded at night).

**Draw order (`0x42532C` / `0x42863C` / `0x426384`).** Rings and ground markers
are drawn after the terrain and **before** the `0x426384` shadow darken, under
the depth-sorted props/actors; markers are ambient-shaded unless the torch is
out. The remake previously drew markers after the lighting pass, over the
actors and unshaded. Reordered to `background -> ambient/light -> rings ->
markers -> shadow darken -> sprites -> projectiles`. The old ledger text calling
this "markers after entities" was wrong.

**Ambient audit (`0x428FE0`).** The threshold table is read as dwords shifted
right by 16, so the comparison at `0x45AE0E+2k` uses entry `k+1`; every branch
body was re-derived and verified live against `0x5076BC`. Two real bugs:
the `[16200,18900)` branch used a runaway positive ramp (neon-green frames) and
the `0x81D608`-gated plates were collapsed; `light_effects` wrongly gated the
ambient itself. Now: exact branch port (`lighting.rs`), ambient always applied
when `day_night != 0`, masks only in the night window `8100..18900` **and** with
`light_effects`, and mask bytes scaled 0..255 -> 0..31 (the previous raw pass
saturated most lit tiles to near-white). `lighting_smoke.mjs` now asserts all
four properties.

**Findings from the first pass (both fixed below).** (1) `0x428C94` camera
target: the original centres the sprite screen anchor `Entity+0xE4/+0xEC`, the
remake centred the ground projection. (2) `0x428E98` strip visibility: the
original draws the inventory strip only after a party cell activates
`_DAT_0081D600`; the remake always showed the leader's. The first-pass
`(-8,-78)` measurement came from screenshots taken while the camera was moved,
so it was not treated as evidence (the code below is).

**Camera port (code evidence, 2026-11-22).** The world loop
(`0x424AF2`, also `0x4366B8`/`0x437FE3`) pushes `[leader+0xEC]`/`[leader+0xE4]`
into `FUN_00428C94` every frame; `FUN_00428C94` assigns
`camera = target - (400,300)` (no easing) and clamps to the per-map terrain
extents (`0x43B045`..`0x43B11B`; loc 4 = 3944/1984, matching live
`0x81D73C`/`0x81D758`). The target uses the **body** anchor record
(`block+0x2F28`), not the composed sprite origin. Ported exactly with
`Res::human_body_anchor`, `Location::camera_bounds` and a per-frame snap. The
screenshot-derived `(-8,-78)` was discarded (the earlier camera was moved during
capture); the fresh oracle run measures a **0 px ring shift** and 97.6% exact
pixels at neutral phase with shadows off.

**Inventory strip (`_DAT_0081D600`, fixed 2026-11-22).** `FUN_00428E98` returns
early without an owner; `0x422D4E` toggles the clicked member's strip;
`0x424376` shows the edited member after the character screen closes;
New Game/load starts hidden. `PlayableApp.inventory_strip_visible` implements the
state, the HUD skips the strip sprites, and the hidden row behaves as world for
hover/clicks/scroll/drag. `tools/strip_smoke.mjs`.

**Ambient palette fade (fixed 2026-11-22).** The ambient delta is **not** an
additive 8-bit shift. `FUN_004392B4` builds per-field ramps with
`f = i * 0.01` accumulated in single precision, and the palette builder
`0x43E3E4` maps each source field through them: `d < 0` ->
`trunc(v * (1 - f))`; `d > 0` -> `v + trunc((max - v) * f)`, with truncation
toward zero (`FUN_0043EAAE` sets the FPU RC bits). Red/blue are 5-bit, green is
the 6-bit 565 field. `lighting.rs::shade_rgb` implements this and replaces the
old `shade_channel`; the background pass and `blit_shaded` use it. The f32
accumulation matters (e.g. `10 * (1 - f(20))` truncates to 7, not 8). Cases
measured live are asserted in `lighting::tests::shade_rgb_matches_the_original_palette_fade`.

**Night-mask re-light (fixed 2026-11-22).** `FUN_0042532C` enters the
`LIGHTS.N` pass only when `_DAT_0082D168 != 0` **and** `light_effects
(0x81D604) != 0` (`0x425354..0x425364`); it walks the visible cells, reads the
low map (`0x650318 + row*0x50 + col`) and two high-map bytes per cell
(`0x617FD8 + row*0xA0 + 2*col`), and draws each non-zero cell's 114x64
`LIGHTS.RES` tile (`0x5A1010 + (id-1)*0x1C80`) through `FUN_0043CC49`. That
blitter does **not** mix toward a warm colour: it rebuilds red/green from the
terrain palette entry by first brightening with the flicker map
(`FUN_0043E7FA(0x82D16C, 0x82D170)` fills `0x479C0C`/`0x4801D0` with
`level + trunc((max-level)*f)`, `max` 63 red / 31 green) and then applying the
**negative** ambient ramp whose index is `|delta| - L`
(`0x479C8C[L] = 0x479D0C + |delta+L|*0x80`, `0x480250[L]` likewise; identity once
`L >= |delta|`), so a lit cell is *less dark* by `L` ramp steps. Blue keeps the
plain ambient ramp `0x5076B8` with no flicker and no `L`. The flicker words are
`0x82D16C = rand()%4+8` / `0x82D170 = rand()%8+0x10`, halved on the default
`0x81D580` path. `lighting::shade_lit` + `lighting::Flicker` port this and
replace the warm-white `light_pixel` mix (now `lighting::torch_pixel`, used only
by the 64x43 torch mask).

**Remaining night-mask residual.** With the ambient and the mask both ported the
oracle pairs are **96.2-97.8% exact in every config at frame 0 and 6000**, and
96.3%/96.7% for `12000`/`18000` with `light_effects=0`. The mask-on pairs move
from 81.8%/83.4% to **84.6%/84.5%** exact (95.6%/96.3% within 2 levels) and the
lit-cell deltas now match measured values (phase 12000 turns `(16,24,0)` into
`(33,36,0)` at `L = 31`). What is left is not a modelling gap: the pass applies
`rand()`-driven flicker to every lit pixel every frame, so lit terrain cannot
match a screenshot pixel-for-pixel, and each oracle config is a separate live
capture with its own actor positions. The per-toggle effect sizes still agree
(mask-on `changed` ~11-14% in both, mask-off 6.1% vs 5.4%).

**Verification.** `cargo test` 637; wasm + manifest rebuilt; smoke, shadow,
lighting, ring, strip, feedback, map, save, combat, mob-AI, ranged, trade all
pass; `parity_index.py --check` OK (the `0x43CC49` row is now `PARITY`). The
oracle pair is a **0 px ring shift** and 97.6% exact at frame 0 with shadows off
(97.2% with shadows); after the palette-fade and mask ports the shaded phases
are 96.2-97.3% exact with `light_effects` off (`12000-nolight` 96.3%,
`18000-nolight` 96.7%) and 84.5-84.6% with the mask on (`12000-all`, `18000-all`;
95.6%/96.3% within 2 levels), the residue being the per-frame random fire
flicker plus live-capture actor motion. The per-toggle effect sizes still agree
(frame 12000 `day_night=0` 73.3% vs ~80%; `light_effects=0` 6.1% vs 5.4%).
