# Settlement producers and barracks training

Reverse-engineered from `konung.res` with REA. This note covers the two
time-based settlement systems: the **producer apprentice** (dialogue
`0x150`/`0x162`/`0x123`) and the **voivode/barracks training** that actually
raises villager levels (`FUN_00417F84`).

## 1. Producer slots are waypoint owners

`Game.N` has no separate producer fields. The words documented as
`Settlement+0x3DA/+0x3E2/+0x3EA/+0x3F2` are the `owner` words of waypoints
`1..4` (waypoint records start at `+0x3D0`, stride 8: `+0` facing, **`+1` role
dialogue id**, `+2` owner, `+4` x, `+6` y):

| waypoint | owner word | role | dialogue opcode |
|---|---|---|---|
| 1 | `+0x3DA` | healer (prod 2) | `0x15A` |
| 2 | `+0x3E2` | merchant (prod 3) | `0x15C` |
| 3 | `+0x3EA` | smith (prod 4) | `0x15B` |
| 4 | `+0x3F2` | voivode (prod 5) | `0x159` |

`FUN_0041182C` returns `slot + 1` for slots `0..4`; the citizen AI
`FUN_004168A4` uses `(archetype & 0x38) >> 3` directly as the waypoint index.

The remake exposes `Settlement::producer(slot)` / `Settlement::voivode()` and
uses them for gameplay. `SettlementTrade::prod_role2..5` is retained only for
save compatibility and the legacy `KONUNG.SAn` export, which derives the four
words from the live waypoints.

### 1.1 Role appointment (`0x14C` / `0x159`–`0x15C`)

`FUN_00431654(slot)` (`0x159`–`0x15C`) verifies the target waypoint
`owner == 0`, finds a party specialist with `FUN_004313B0(slot)`, settles them
with `FUN_00432954`, then:

- clears `archetype & 0x38` (the waypoint owner becomes the role marker);
- writes `Entity+0xD0 = waypoint[slot].+1` (**the authored role dialogue id** —
  this is what makes the new healer/merchant/smith/voivode talkable);
- sets `flags_a = 0x60`, `flags_b &= !0x04`, destination `+0x0C/+0x0E =
  waypoint x/y`, and pathfinds to the station.

`FUN_00432954` (`0x14C`) settles a party NPC into the settlement group. If
waypoint 0 is free it also writes `dialogue_id = waypoint[0].+1`, clears the
role bits and sets `flags_a = 0`. The elder's authored dialogue id is the
village-specific management dialogue (70, 105, 111, …).

The remake keeps recruited companions as a `party` overlay over their source
`tables.entities` row, so settlement scans subtract `party_ids` from worker /
defender / building / apprentice / restock / tribute counts. This reproduces the
original `FUN_00431820` group compaction without changing canonical entity ids.

**Combat death does not clear a role waypoint.** Only `0x16B` (scripted removal),
`0x123` (recruit) and `0x14C`/`0x159`–`0x15C` change owners.

## 2. Producer apprentice (`0x150` / `0x162` / `0x123`)

* `0x150` (`FUN_00432804`) is a predicate only: settlement flag `0x10`, money
  ≥ 100 and an eligible candidate.
* `0x162` (`FUN_00434364`) takes the first settlement-group member with
  `visual(+0xD6) < 3`, `archetype & 0x38 == 0`, not hidden (`&0x80`),
  `kind != 3/9` and `FUN_0041182C == 0` (owns no home waypoint). It ORs the
  dialogue NPC's role bits (`0x08`/`0x10`/`0x18`), clears the order byte,
  resets the action and stores `_DAT_0081D640` at `+0x3FC/+0x400/+0x404`.
* Movement (`FUN_004168A4` role branch): the actor walks to `waypoint[role]`
  and stops wandering. If it is already on one of the 8 neighbours
  (`FUN_0041165C`) it faces the waypoint's facing; otherwise it moves to the
  first terrain-free neighbour (`FUN_00410FFC`). It never stands on the
  master's own cell.
* `0x123` (`FUN_00431820`) applies the profession gain when the apprentice
  later joins the party: `gain = (elapsed_frames / 16) / 0x5A0`, factor `0.03`
  (`FUN_0043eaae` is a plain round):
  * role 1 → healing `+0x48`: `round(healing + 0.03*gain)`, or
    `round(healing + (healer.healing - healing)*0.03*gain)` when the healer
    exists; clamp 500.
  * role 2 → trade `+0x42`: `trade += gain`, capped by the merchant's trade
    and 100.
  * role 3 → smithing `+0x4C`: the role-1 formula against the smith; no clamp.

## 3. Barracks training (`FUN_00417F84`) — the level-ups

`FUN_0041D048` runs the settlement branch once per **16 original frames**
(`(frame + 7) & 0xF == 0`), for **all 20 settlements**, after the role-2
(`FUN_004175FC`) and role-4 (`FUN_004179C8`) producer passes, and only when that
pass produced no target/prop event.

`FUN_00417F84(settlement)`:

1. Skip when `Settlement+0x3F2` (voivode = `waypoint[4].owner`) is 0.
2. `Settlement+0x0C` -= 1; if still ≥ 1, stop. Otherwise reset to `0x2D0`
   (720 passes × 16 frames × 60 ms ≈ 11.5 min; the first pass fires
   immediately because the field starts at 0).
3. For each entity in `Group[settlement.group]`:
   * skip hidden/corpse (`archetype & 0x80`), `kind == 3/9`, `visual >= 3`, and
     the voivode itself;
   * `+0x33` level += 1 when `FUN_00410B08(ent, 0)` allows it (class minima,
     level != 13);
   * for dex/cha/str/end (`+0x56/+0x5A/+0x60/+0x64`) below the voivode's
     value: add 1000 XP, pay `FUN_00410B08(ent, stat)`, stat += 1, remove the
     1000 XP (the raise is free but `+0x3C` invested XP records the cost);
   * `+0x33` level += 1 again;
   * if any stat deficit: `melee(+0x40) += (voivode - own)/deficit + 1`, same
     for archery `+0x44`, then `FUN_0041C69C` (derived = stored when no
     temporary effect is active).

The stat caps/minima are the `0x45B0AB` table (`[class][stat][level]{cap,min}`,
`class = (archetype & 7) - 1`, or `1` for `archetype & 0x40`), and the costs are
the `0x45AFB0` 13-tier table. The specialization byte table at `0x45AE28` is
`[3, 6, 2, 3, 5]` = charisma, endurance, dexterity, charisma, strength.

Shipped `Game.1`: 14 of the 15 active settlements already own `waypoint[4]`
(e.g. loc 19 → 187, loc 22 → 217, loc 24 → 329, loc 37 → 475); loc 4 has none.

## 4. The full settlement block (`FUN_0041D048`)

The same 16-frame branch runs, per settlement, in this order:

```text
free   = members: visual<3, visible, kind != 3/9
roles  = free with archetype & 0x38
owners = nonzero waypoints[0..4].owner
workers = group.activity_1d == 1 ? 0 : free - (roles + owners)

if free == 0:
    group.activity_1d = 0
    settlement.flags_4b2 &= 0xEA
    settlement.+0x14 = 0

if group.activity_1d == 1 and settlement.flags_4b2 & 0x80 == 0:
    party_power / defender_power >= 2.0  -> defeat

if settlement.loc_id == current_map:
    FUN_00416670(settlement)          # waypoint claim / alert

if settlement.flags_4b2 & 0x10 == 0:
    settlement.+0x14 += 1
    if settlement.+0x14 >= 0x2760:
        settlement.money_pool = round(
            money_pool + group.entity_count*5
            + settlement.target_base0*20*sqrt(leader.trade) + 1)
        settlement.+0x14 = 0

local_30 = target_timer_pass(settlement, workers)
FUN_004175FC(settlement)
FUN_004179C8(settlement)
if local_30 == 0: FUN_00417F84(settlement)
```

**Target timer pass.** Each target is `+0 key u8, +1 state u8, +2 prop_index u16,
+6 timer u16` (stride 8). `value = 0x458A02[key].u16_at_+4`
(`SETTLEMENT_TARGET_TABLE`; the `+4`/`+6` columns are identical for the 16
shipped keys). The timer is a plain countdown:

```text
key >= 0x80 or key >= 16: ignore
timer > 0, state < 3, workers > 0:
    timer -= 1; if timer == 0:
        state += 1
        if state != 3: changed = true; timer = value*60/workers
        update_prop()
timer > 0 (other states / no workers):
    timer -= 1; if timer == 0:
        state += 1
        if state != 6: timer = value
        update_prop()
timer == 0, state == 6, workers > 0:
    changed = true; state = 0; timer = value*60/workers; update_prop()
```

`update_prop` is `FUN_004172AC` (only when `settlement.loc_id == current_map`):
the prop object becomes `base(key) + state + 30`; the prop's palette is copied
from the new object record (`+0x14`), and its frame/FLAMES phases are
reinitialised. The original also maintains the object refcount/active list
(`0x81B954`/`0x81C194`) and picks the frame/flame variants with `rand()`; the
remake resolves sprites directly and uses a deterministic variant hash, so only
the visible object/palette/frame/flames are modelled. `changed` (`local_30`)
gates the barracks training.

**Waypoint claim (`FUN_00416670`).** Only when `settlement.flags_4b2 & 5 != 0`.
For each party member (leader + companions) inside `Group.rect_a`:
`flags & 1` claims immediately; otherwise a member whose visual differs from
`Group.class_1e & 3` and who has equipment in slots 0..2 or an inventory item of
class < 3 claims. A claim sets `Group+0x1A = 0` (party group) and
`Group+0x1D = 1`; otherwise `Group+0x1D = 0`.

**Defeat.** `FUN_0041B658` power is `0.1*armor + 0.2*dex + 0.3*strength +
0.3*endurance + 0.002*melee + 0.003*archery + 0.2*weapon rating`. The check sums
it over the party group and over the settlement members (visual<3, visible,
not in a death action row). A zero defender sum is an infinite ratio (defeat
when the party has any power). On defeat: `Group+0x1D = 0`; every member is
disarmed/idled. `Entity+0x00` is the **action row**, not a kind: rows `3`/`9`
are the death animations. Such actors get `Entity+0x10 = Entity+0xD7 - 1`
(freeze on the last frame of the current death action) and
`archetype |= 0x80` (hidden corpse); live members just go idle. Settlement
flags `|= 0x80`, `&= 0xEA`, `+0x14 = 0`, money pool = 0; the party stops
fighting; `DEFEAT<location>.FLM` plays.

The remake stores the live action row in `ActorMotion.action` and the dead flag
in `ActorMotion.dead`; the parsed `Entity.kind` is the authored/initial row and
is only a fallback for locations without a population cache. `runtime_action`
resolves the live row, the worker/defender/training filters use it, and the
defeat snaps `motion.frame = death_frames - 1` (`death_frames` = the current
death row's `+0xD7`, or `action_cycle_frames` for the stance's death row).

**Random event.** When settlement flag `0x10` is clear, the `+0x14` counter
increments; at `0x2760` it adds
`round(entity_count*5 + target_base0*20*sqrt(leader.trade) + 1)` to the money
pool and resets. `FUN_0043EB2E` is `sqrt` (not a random helper).

## 5. Remake mapping

* `app/settlement_pass.rs` — the `FUN_0041D048` block, one pass per 16 frames.
* `app/settlement_targets.rs` — worker count, target timer/state machine,
  `local_30`.
* `app/settlement_defense.rs` — no-worker clear, `FUN_00416670` claim, defeat,
  random event.
* `app/world_items.rs::advance_world_frame` — fires `tick_settlement_pass(1)` on
  `world_frame % 16 == 9`; `apply_settlement_prop` is the live `FUN_004172AC`.
* `app/settlement_trade.rs` — role-2/role-4 restock.
* `app/settlement_training.rs` — the voivode pass, skipped when `local_30` is set
  (the timer is not decremented either).
* `actors/motion/citizen.rs` — the adjacent-cell station movement.
* `app/dialogue_ops.rs` — `0x123` role gains and the `Settlement::producer`
  lookups.
* `character/class.rs` — raw-class-index `can_raise_any_index` /
  `raise_cost_index`.

All 20 settlements are processed regardless of the current location; only the
waypoint claim and the live prop update are current-map gated, matching the
original. `Settlement+0x14` is modelled by `Settlement::progress_14` (the
counter never exceeds `0x2760` before reset, so the low 16 bits suffice).
