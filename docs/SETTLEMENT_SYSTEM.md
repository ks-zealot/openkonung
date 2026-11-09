# Konung settlement / village system

Target: `konung.res` (`sha256 d698c4f231489a372cc25212ab2819720a36d7ce5307a045db684135cf679810`),
`Game.1`'s `Settlement[20]` table, and `QUEST_R.RES` (`Quest_r.res`).

This note covers the four player-facing village systems:

1. **Conquest** — claim, force defeat, and quest-gated peaceful transfer.
2. **Roles / job promotion** — elder, healer, merchant, smith, voivode.
3. **Tribute** — the settlement money pool and the “забрать дань” option.
4. **Buildings** — the elder build menu, construction timers and prop sprites.

The recurring data model is the `Settlement` record (`Game.N +0xA769A`, stride
`0x4B5`) and its eight waypoints at `+0x3D0` (stride 8):

```text
+0  u8   facing
+1  u8   role dialogue id  (FUN_00431654 / FUN_00432954 copy this to Entity+0xD0)
+2  u16  owner Entity index (0 = unassigned)
+4  u16  station x (runtime/Entity grid)
+6  u16  station y
```

The first five waypoints are the village roles; the remaining slots are extra
stations used by the citizen AI. `FUN_0041182C` returns `slot + 1` for slots
`0..4` (`1 староста`, `2 знахарь`, `3 купец`, `4 кузнец`, `5 воевода`).

---

## 1. Conquest

`FUN_0041D048` runs once per 16 original frames (frame `%16 == 9`) for all 20
settlements. Relevant branches, in order:

```text
free   = visible members with visual(+0xD6) < 3
roles  = free members with archetype & 0x38
owners = nonzero waypoints[0..4].owner
workers = Group+0x1D == 1 ? 0 : free - (roles + owners)

if free == 0:
    Group+0x1D = 0
    Settlement.flags_4b2 &= 0xEA
    Settlement.+0x14..+0x17 = 0

if Group+0x1D == 1 && Settlement.flags_4b2 & 0x80 == 0:
    party_power / defender_power >= 2.0 -> defeat

if Settlement.loc_id == current_map:
    FUN_00416670(settlement)        # claim / wake

if Settlement.flags_4b2 & 0x10 == 0:
    Settlement.+0x14 += 1           # tribute accumulator
```

### 1.1 Claim (`FUN_00416670`)

Only runs when `flags_4b2 & 5 != 0`. For each party member inside group rectangle
A:

- `flags_4b2 & 1` (hostile) claims immediately;
- otherwise the member must have a different visual family (`Entity+0xD6 !=
  Group.class & 3`) **and** be armed: equipment slots 0..2 non-zero, or an
  inventory item of class `< 3`.

A claim sets `Group+0x1A = party group (0)` and `Group+0x1D = 1`; otherwise
`Group+0x1D = 0`.

### 1.2 Force defeat (`FUN_0041B658` / `FUN_0041D048`)

Party power is summed over group 0; defender power is summed over settlement
members with `visual < 3`, not hidden/corpse, not in action row `3`/`9`.
A zero defender sum is an infinite ratio (defeat when the party has any power).

On defeat:

- `Group+0x1D = 0`;
- dead members freeze on the last death frame, `archetype |= 0x80`, `flags_a = 0`;
- live members idle (`flags_a = 0`, order/path cleared);
- `Settlement.flags_4b2 |= 0x80`, then `&= 0xEA`; progress and money pool zero;
- `DEFEAT<location>.FLM` is requested.

### 1.3 Peaceful / quest transfer (`0x16D`, `FUN_00434310`)

`0x16D` does:

```text
Settlement.flags_4b2 &= 0xEA       # clears 0x01/0x04/0x10/0x15
Settlement.+0x14 = 0
Settlement.+0x10 = 0
```

`0xEA` is exactly `!0x15`; flag `0x10` means **independent / not yet yours**.
Clearing it is what enables tribute accrual and changes merchant dialogue and
trade role pricing.

### 1.4 Shipped `Game.1` conquest routes

| Set | Location | Elder dialogue | Gate / route |
|---|---|---|---|
| 0 | 14 Верхний лагерь | 164 | No `0x16D`; boss dialogue 164 entry 4693 runs `0x16B` after victory |
| 1 | 19 Черный Бор | 70 | Endgame flags 13, 81, 61, 66, 49; entry 1019 `0x16D` |
| 2 | 22 Поречье | 154 | `!flag 69`, `op_0x146` (max building ≥ 5); entry 4328 `0x16D` |
| 3 | 27 Бережки | 105 | `flag 14`, `flag 134`; entry 2203 `0x16D` |
| 4 | 29 Камни | 119 | `flag 21`, `!flag 161`, `op_0x10F` (class 11 subtype 0x33); entry 2875 `0x16D` |
| 5 | 4 Старый лагерь | 140 | `!flag 109`; entry 3766 `0x16D` (starting village) |
| 6 | 33 Борье | 123 | `flag 145`, `!flag 142`, `!flag 144`; entry 3060 `0x16D` |
| 7 | 35 Средний лагерь | 111 | `flag 16`, `!flag 84`; voivode chain; entry 2430 `0x16D` |
| 8 | 24 Куницын Бор | 155 | `!flag 172`, `flag 49` + `op_0x50`; entry 4404 `0x16D` |
| 9 | 1 Родники | 151 | `op_0x12E` (flag 0x10 set), `!flag 79`, `op_0x38`; entry 4177 `0x16D` |
| 10 | 11 Ловье | 152 | `!flag 62`; entry 4231 `0x16D` |
| 11 | 7 Византийский Лагерь | 168 | No `0x16D`/`0x16B`; not player-conquerable via dialogue |
| 12 | 16 Лесовье | 169 | `flag 17`, `!flag 196`, `op_0x130` + `op_0x13F`; entry 4826 `0x16D` |
| 13 | 37 Нижний лагерь | 170 | `!flag 202`, `!flag 24`; entry 4857 `0x16D` + 500 money |
| 14 | 39 Лесной лагерь | 171 | `!flag 193`, `op_0x5E`; entry 4935 `0x16D` |

The low family opcodes used above are location checks: `op_0x38 = location 1`,
`op_0x45 = 14`, `op_0x50 = 25`, `op_0x5C = 37`, `op_0x5E = 39` — all
“no live object-group member in that location”.

**Combat death does not clear a role waypoint.** Only `0x16B` (scripted removal,
`FUN_00433C94`), `0x123` (recruit, `FUN_00431820`) and `0x14C`/`0x159`–`0x15C`
(assignment) change waypoint owners. The guide-style “kill the elder and replace
him” is therefore only faithful when a dialogue action removes him.

---

## 2. Roles / job promotion

### 2.1 Role table

| Waypoint | Role | Owner word | Authored `+1` dialogue | Opcode | Role bits | Skill gate |
|---|---|---|---|---|---|---|
| 0 | староста | `+0x3D2` | village elder dialogue | `0x14C` | none | any party NPC; slot free |
| 1 | знахарь (prod 2) | `+0x3DA` | 101 (loc 4: 149) | `0x15A` | `0x08` | healing > 100 |
| 2 | купец (prod 3) | `+0x3E2` | 103 (loc 27: 188, loc 1: 148) | `0x15C` | `0x10` | trade > 30 |
| 3 | кузнец (prod 4) | `+0x3EA` | 102 (loc 7: 117) | `0x15B` | `0x18` | smithing > 4 |
| 4 | воевода (prod 5) | `+0x3F2` | 100 (loc 37: 177, loc 39: 178) | `0x159` | `0x20` | strength_base > 10 and melee > 10 |

The producer words `+0x3DA/+0x3E2/+0x3EA/+0x3F2` are literally
`waypoint[1..4].owner`; `Settlement::producer(slot)` uses the 0-based waypoint
index. The shop/merchant is role 3 (waypoint 2), the smith role 4 (waypoint 3).

### 2.2 Flow

1. **Source village apprentice**: talk to a role NPC. The option is gated by
   `0x150` (`FUN_00432804`: flag `0x10` set, player money ≥ 100, an eligible
   candidate exists). `0x162` (`FUN_00434364`) marks the first eligible
   settlement member: `visual < 3`, no role bits, visible, `kind != 3/9`, no
   owned home waypoint. It ORs the producer role bits (`0x08`/`0x10`/`0x18`) and
   stores the world frame in `Settlement+0x3FC/+0x400/+0x404`.
2. **Recruit the apprentice**: their own dialogue uses `0x123`
   (`FUN_00431820`) when the party is not full. The elapsed time grants healing
   / trade / smithing (see `SETTLEMENT_TRAINING.md`).
3. **Assign in the target village**: talk to the elder, “о делах деревни”, then
   the role option (`0x159`–`0x15C`). `FUN_00431654` checks the waypoint owner is
   free, finds the party specialist with `FUN_004313B0`, settles them
   (`FUN_00432954`) and then:
   - `Entity+0xD0 = waypoint[slot].+1` — the authored role dialogue id;
   - `archetype &= !0x38` — clear apprentice role bits (waypoint owner is now the
     authoritative role marker);
   - `flags_a = 0x60`, `flags_b &= !0x04` — party/peace flags;
   - destination `+0x0C/+0x0E = waypoint x/y`, then pathfind/walk.
4. **Elder assignment**: `0x14C` (`FUN_00432954`) settles a party NPC into the
   settlement group; if waypoint 0 is free it also writes `Entity+0xD0 =
   waypoint[0].+1`, clears role bits and sets `flags_a = 0`.

The remake ports this as `opcode 0x14C` / `0x159`–`0x15C` in
`app/dialogue_ops.rs`. The new specialist's station is currently snapped to the
waypoint cell rather than walked there; the sprite/role/dialogue behaviour is
faithful.

### 2.3 Party overlay note

The remake keeps recruited companions in `app.party` while leaving their source
`Entity` row in `tables.entities` (the original `FUN_00431820` compacts the old
group). Settlement scans that walk a group range therefore subtract
`party_ids`: worker count, defender power, max building, apprentice candidate,
role-4 distribution and the tribute `entity_count`. This preserves the original
“recruited villagers no longer belong to the village group” semantics without
changing canonical source ids (legacy export patches party records back into
`Game.N` by index).

---

## 3. Tribute

`Settlement+0x10` is the money pool and `+0x14` the progress counter. In
`FUN_0041D048`, when `flags_4b2 & 0x10 == 0` (village is yours):

```text
Settlement.+0x14 += 1
if Settlement.+0x14 >= 0x2760:
    Settlement.money_pool += round(
        group.entity_count * 5
        + settlement.target_base0 * 20 * sqrt(leader.trade)
        + 1)
    Settlement.+0x14 = 0
```

`0x2760` is 10080 original frames = 10080 × 60 ms ≈ **10.08 real minutes**. The
“come back in a week” dialogue line is flavour text.

The elder management page exposes “Я бы хотел забрать дань” only when `0x137`
(`money_pool != 0`). Selecting it runs `0x11B`: `player.money += pool`,
`pool = 0`. `0x11A` (repay 120) and `0x11C` (+100 loan, sets flag `0x20`) are the
merchant loan/repayment, not village tribute.

`0x16D` clears flag `0x10`, which enables tribute accrual after a peaceful
conquest. A force defeat also clears it (`flags_4b2 &= 0xEA`).

---

## 4. Buildings

### 4.1 Build menu

The elder management page 139 (“Я думаю, имеет смысл что-нибудь построить”) is
gated by `0x148` (OR of the requirement predicates). Each option writes the
target timer and then the 16-frame pass advances the target state:

| Target | Building | Timer write | Requirement |
|---|---|---|---|
| 1 | Изба знахаря | `0x163` | `0x151` |
| 2 | Лавка | `0x167` | `0x155` |
| 3 | Кузница | `0x166` | `0x154` |
| 4 | Казарма | `0x164` | `0x152` |
| 5 | Мост | `0x168` | `0x156` |
| 6 | Колодец | `0x165` | `0x153` |
| `base0+7 .. +base1` | Частокол | `0x169` | `0x157` |

`0x146` additionally gates the menu on max group building skill ≥ 5. Each
requirement predicate requires `target.state == 0`, `target.timer == 0`, and
`FUN_00431520(group)` (max `Entity+0x46` building) ≥ the key's requirement word
at `0x458A02[key].+0`.

### 4.2 Target state machine

A target is `+0 key u8, +1 state u8, +2 prop_index u16, +4 unused, +6 timer u16`
(stride 8). Timer value = `0x458A02[key].u16@+4` (all 16 shipped keys have
identical `+4/+6` values).

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

Interpretation: **0→1→2→3 is construction**, **3 is complete**, **4→5→6 is
damage/ruin**, **6→0 rebuilds**. Worker scaling means more free villagers build
faster; role holders and waypoint owners reduce `workers`.

### 4.3 Prop sprites

`FUN_004172AC` (live) and `FUN_0043ADF0` (map load) set the prop object id to:

```text
object_id = 0x458A02[key].base(u16@+2) + target.state + 30
```

The `+30` is the placement/object-table offset; verified against `Konung.N`
props (e.g. loc 19 target key 1 base 53, state 3 → object 86; prop 373 stores
raw 56 → object 86). The prop's palette is copied from object record `+0x14` and
its frame/FLAMES phases are reinitialised. The original randomises those phases
with `rand()`; the remake uses a deterministic variant hash.

---

## 5. Remake mapping

| Area | Code |
|---|---|
| Claim / defeat / no-worker / tribute event | `app/settlement_defense.rs` |
| Worker count / target state machine / `local_30` | `app/settlement_targets.rs` |
| Full 16-frame pass | `app/settlement_pass.rs` |
| Target → prop linkage | `app/world_items.rs::apply_settlement_prop(s)` |
| Role opcodes `0x14C` / `0x159`–`0x15C` / `0x123` / `0x162` | `app/dialogue_ops.rs` |
| Waypoint parse (`+1` dialogue id) | `population/parse.rs`, `population/types.rs` |
| Save rehydration of `+1` | `app/save.rs` |
| Legacy export of `+1` | `legacy_save.rs::patch_settlements` |
| Tribute `0x137` / `0x11B` / `0x16D` | `app/dialogue_ops.rs` |
| Building tests | `app/tests/settlement.rs`, `app/tests/ops.rs` |

See also `SETTLEMENT_TRAINING.md` for the producer-apprentice gain and the
voivode/barracks training pass.
