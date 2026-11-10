# Konung quest/dialogue script opcodes — reference

Complete catalogue of the opcodes used by the `QUEST_R.RES` dialogue
**condition** (`B4`) and **action** (`B5`) programs, recovered from `konung.res`
(Ghidra 12.1.4; sha256 `d698c4f2…679810`). The machine-readable source of truth
is [`web/src/script.rs`](../src/script.rs); this document mirrors it and adds the
low-opcode families, the usage census and the execution model.

## 1. Program encoding and dispatch

A program is a `u16` list **terminated by `0xFFFF`**. Each word carries an operand
in its low 9 bits and flags in the high bits:

| bits | meaning |
|---|---|
| `0x01FF` | operand |
| `0x2000` | operand is a **script opcode** → `FUN_0043159C(operand)` |
| `0x4000` | condition group separator (only the condition evaluator `FUN_00434FE8`) |
| `0x8000` | negate the term / set the `B7` flag slot `operand` |

A word with `0x2000` clear and no `0x2000`/`0x4000` is a `B7` flag test/set. The
**same opcodes are shared** by conditions (`FUN_00434FE8`) and actions
(`FUN_00434BB8`); a handler that only makes sense as a test is a *predicate*, one
with a side effect is an *action*.

`FUN_0043159C(op)`:

```text
op < 0x38  -> FUN_00432F8C(op + 1)      # group predicate      (N = op+1)
op < 0x70  -> FUN_00433FF8(op - 0x37)   # group member predicate (N = op-0x37)
op < 0xA6  -> FUN_0043395C(op - 0x6F)   # remove 0xB item      (N = op-0x6F)
op < 0xDC  -> FUN_004347D0(op - 0xA5)   # create 0xB item      (N = op-0xA5)
op < 0x112 -> FUN_00433A8C(op - 0xDB)   # has 0xB item         (N = op-0xDB)
else          CALL [op*4 + 0x465BD4]    # op-indexed handler table
```

The `0x465BD4` table is **indexed by `op`**, so its entries below `0x112` are
unused filler; the real table begins at `0x46601C`. Handlers exist for
`0x112..0x16E` (higher entries are zero/unused).

## 2. Low-opcode families (`0x00..0x111`)

These are *parameterized families*: the opcode selects the function family and
the operand `N` selects the group/class/subtype.

| op range | handler | N | kind | meaning |
|---|---|---|---|---|
| `0x00..0x37` | `FUN_00432F8C` | `op+1` | predicate | group/location `N` has an active member (group table `0x67C08C`, stride `0x10`; `+0x0C` high byte == N, `+0x0F & 0x40`, then `+0x0E < +0x0C` or a member with `archetype & 0x80`) |
| `0x38..0x6F` | `FUN_00433FF8` | `op-0x37` | predicate | group `N` member-state predicate (finds no non-hidden/live member) |
| `0x70..0xA5` | `FUN_0043395C` | `op-0x6F` | action | remove inventory item class `0x0B`, subtype `N-1`; mark the instance consumed (`class = 0xFF`) and compact the 32-slot inventory |
| `0xA6..0xDB` | `FUN_004347D0` | `op-0xA5` | action | create item instance class `0x0B`, subtype `N-1`; insert at inventory front, or drop it at the player cell when the bag is full |
| `0xDC..0x111` | `FUN_00433A8C` | `op-0xDB` | predicate | inventory contains class `0x0B`, subtype `N-1`; returns `slot+1` (truthy) |

Verified from `konung.res` (REA, 2026-10-22): the `0xB` items are the
party/quest token family, so these families are **inventory** operations, not
entity spawn/removal operations.

`N` domains: group ops `1..0x38`; `0xB` inventory item ops `1..0x36`.

Usage census (shipped dialogues): 89 of these opcodes are used, total 231
references:

| family | distinct ops used | references |
|---|---|---|
| `0x00..0x37` | 2 (`0x0`,`0x3`) | 6 |
| `0x38..0x6F` | 12 | 62 |
| `0x70..0xA5` | 20 | 34 |
| `0xA6..0xDB` | 30 | 40 |
| `0xDC..0x111` | 25 | 89 |

## 3. High-opcode table (`0x112..0x16E`)

93 opcodes are defined (85 used). `uses` = references across all 300 dialogues in
the shipped `QUEST_R.RES`.

| op | dec | handler | kind | uses | effect |
|---|---|---|---|---|---|
| 0x112 | 274 | `0x431DCC` | Action | 45 | activate the target NPC's group (`Group+0x1D = 1`); if settlement group matches, `flags_4b2 |= 0x15` |
| 0x113 | 275 | `0x431E2C` | Action | 8 | trade/loot: load player+partner inventories/money, open exchange mode 7 |
| 0x114 | 276 | `0x4346B0` | Action | 3 | set protagonist +0xD3 = 0x46 |
| 0x115 | 277 | `0x433F28` | Predicate | 118 | rng()%100 < 50 |
| 0x116 | 278 | `0x4337A8` | Predicate | 8 | skill/stat check FUN_004313B0(4) |
| 0x117 | 279 | `0x432F10` | Predicate | 2 | skill/stat check FUN_004313B0(1) |
| 0x118 | 280 | `0x4335EC` | Predicate | 6 | skill/stat check FUN_004313B0(3) |
| 0x119 | 281 | `0x4330C4` | Predicate | 2 | skill/stat check FUN_004313B0(2) |
| 0x11A | 282 | `0x434B68` | Both | 3 | settlement && money >= 120: `flags_4b2 &= ~0x20`, pay 120 |
| 0x11B | 283 | `0x433A40` | Action | 1 | `money += settlement[+0x10]`; zero the settlement money pool |
| 0x11C | 284 | `0x433824` | Action | 3 | if settlement: `flags_4b2 |= 0x20`, +100 gold (returns false) |
| 0x11D | 285 | `0x433868` | Action | 0 | gold -5 |
| 0x11E | 286 | `0x433890` | Action | 5 | gold -10 |
| 0x11F | 287 | `0x4338B8` | Action | 3 | gold -20 |
| 0x120 | 288 | `0x4338E0` | Action | 19 | gold -50 |
| 0x121 | 289 | `0x433908` | Action | 24 | gold -100 |
| 0x122 | 290 | `0x433930` | Action | 24 | gold -250 |
| 0x123 | 291 | `0x431820` | Action | 44 | insert target NPC into party group 0 and apply role training bonuses |
| 0x124 | 292 | `0x43453C` | Both | 0 | add quest item (class 8/0x12); true if slot free |
| 0x125 | 293 | `0x4346DC` | Action | 0 | gold +5 |
| 0x126 | 294 | `0x434704` | Action | 2 | gold +10 |
| 0x127 | 295 | `0x43472C` | Action | 0 | gold +20 |
| 0x128 | 296 | `0x434754` | Action | 1 | gold +50 |
| 0x129 | 297 | `0x43477C` | Action | 9 | gold +100 |
| 0x12A | 298 | `0x4347A4` | Action | 7 | gold +250 |
| 0x12B | 299 | `0x4345E0` | Action | 4 | grant 10 XP |
| 0x12C | 300 | `0x434610` | Action | 2 | grant 20 XP |
| 0x12D | 301 | `0x434640` | Action | 109 | grant 50 XP |
| 0x12E | 302 | `0x43425C` | Predicate | 353 | settlement flag 0x10 set |
| 0x12F | 303 | `0x432D64` | Predicate | 7 | protagonist has dialogue id != 0xFF |
| 0x130 | 304 | `0x433760` | Predicate | 5 | settlement word +0x3F2 != 0 |
| 0x131 | 305 | `0x4330F8` | Predicate | 7 | money > 29 |
| 0x132 | 306 | `0x43312C` | Predicate | 0 | money >= 40 |
| 0x133 | 307 | `0x433160` | Predicate | 3 | money >= 50 |
| 0x134 | 308 | `0x433194` | Predicate | 20 | money >= 100 |
| 0x135 | 309 | `0x4331C8` | Predicate | 10 | money >= 500 |
| 0x136 | 310 | `0x4331FC` | Predicate | 12 | money >= (npc level > 3 ? 500 : 300), halved when settlement flag 0x10 is clear |
| 0x137 | 311 | `0x4336A4` | Predicate | 1 | settlement[+0x10] != 0 |
| 0x138 | 312 | `0x4336E8` | Predicate | 11 | current settlement present |
| 0x139 | 313 | `0x433454` | Predicate | 1 | owns item class 9 subtype 1 |
| 0x13A | 314 | `0x432EC8` | Predicate | 14 | settlement word +0x3DA != 0 |
| 0x13B | 315 | `0x432F44` | Predicate | 2 | settlement[+0x1E]>>24 == 3 |
| 0x13C | 316 | `0x433718` | Predicate | 2 | settlement[+0x36]>>24 == 3 |
| 0x13D | 317 | `0x4337DC` | Predicate | 2 | settlement[+0x46]>>24 == 3 |
| 0x13E | 318 | `0x4334D8` | Predicate | 1 | owns item class 9 subtype 2 |
| 0x13F | 319 | `0x4335A4` | Predicate | 5 | settlement word +0x3EA != 0 |
| 0x140 | 320 | `0x432CE0` | Predicate | 2 | settlement[+0x2E]>>24 == 3 |
| 0x141 | 321 | `0x43307C` | Predicate | 2 | settlement word +0x3E2 != 0 |
| 0x142 | 322 | `0x43355C` | Predicate | 2 | settlement[+0x26]>>24 == 3 |
| 0x143 | 323 | `0x432DA0` | Predicate | 0 | settlement[+0x3E]>>24 == 3 |
| 0x144 | 324 | `0x433264` | Predicate | 3 | carries item with template flag 0x659B1A bit0 |
| 0x145 | 325 | `0x432D28` | Predicate | 5 | protagonist[+0xCF]>>24 >= 0 |
| 0x146 | 326 | `0x432DE8` | Predicate | 3 | settlement group max building skill (`FUN_00431520`) >= 5 |
| 0x147 | 327 | `0x432E48` | Predicate | 0 | target[base0+7] state/timer nonzero (and target count byte base1 != 0) |
| 0x148 | 328 | `0x43339C` | Predicate | 4 | any requirement predicate 0x151..0x157 holds |
| 0x149 | 329 | `0x433620` | Predicate | 1 | owns item class 9 subtype 0 |
| 0x14A | 330 | `0x4341F0` | Predicate | 22 | protagonist[+0x5A]>>16 >= 1600 |
| 0x14B | 331 | `0x433EFC` | Action | 3 | recalc protagonist FUN_0041BA08 |
| 0x14C | 332 | `0x432954` | Action | 17 | settle the target NPC into the settlement group; if waypoint 0 is free, copy its `+1` role dialogue id to `Entity+0xD0`, clear role bits, `flags_a=0` |
| 0x14D | 333 | `0x4340E8` | Predicate | 12 | settlement flag 0x20 set |
| 0x14E | 334 | `0x433B18` | Action | 5 | target healing `+0x48 = round((1600-player.hp)/1024 + healing)`, target hp = 1600 |
| 0x14F | 335 | `0x433BA0` | Action | 6 | each group member: `healing = round((640-hp)/1024 + target.healing)`, hp = 1600 |
| 0x150 | 336 | `0x432804` | Predicate | 7 | settlement flag 0x10 + money>=100 + eligible apprentice (visual<3, no role/home waypoint) |
| 0x151 | 337 | `0x432544` | Predicate | 1 | requirement 0x458A06 for state settlement[+0x1D]>>24 |
| 0x152 | 338 | `0x4326A4` | Predicate | 1 | requirement for state settlement[+0x35]>>24 |
| 0x153 | 339 | `0x432754` | Predicate | 4 | requirement for state settlement[+0x45]>>24 |
| 0x154 | 340 | `0x432314` | Predicate | 1 | requirement for state settlement[+0x2D]>>24 |
| 0x155 | 341 | `0x4325F4` | Predicate | 1 | requirement for state settlement[+0x25]>>24 |
| 0x156 | 342 | `0x4323C4` | Predicate | 1 | requirement for state settlement[+0x3D]>>24 |
| 0x157 | 343 | `0x432474` | Predicate | 2 | requirement for group record slot settlement[0]+7 |
| 0x158 | 344 | `0x434670` | Both | 3 | if owns item class 0xB subtype 0x22: +0xD2=0x13 |
| 0x159 | 345 | `0x433FD4` | Both | 3 | station a party specialist at waypoint 4 (voivode); copy waypoint `+1` dialogue id, clear role bits |
| 0x15A | 346 | `0x433F68` | Both | 1 | station a party specialist at waypoint 1 (healer) |
| 0x15B | 347 | `0x433FB0` | Both | 2 | station a party specialist at waypoint 3 (smith) |
| 0x15C | 348 | `0x433F8C` | Both | 1 | station a party specialist at waypoint 2 (merchant) |
| 0x15D | 349 | `0x434168` | Predicate | 45 | party capacity <= current group member count (at capacity) |
| 0x15E | 350 | `0x434228` | Predicate | 47 | target NPC +5 == 0 |
| 0x15F | 351 | `0x434130` | Predicate | 0 | dialog NPC[+0x5A]>>16 > 800 |
| 0x160 | 352 | `0x4342A4` | Predicate | 7 | target NPC[+0x30]>>24 > 3 |
| 0x161 | 353 | `0x4342DC` | Predicate | 47 | dialog NPC[+4] & 0x38 != 0 |
| 0x162 | 354 | `0x434364` | Action | 14 | mark eligible settlement member as apprentice for the dialog NPC's producer role (no party join) |
| 0x163 | 355 | `0x4321C4` | Action | 1 | target1.timer = table[target1.key] (`+0x458A06`) |
| 0x164 | 356 | `0x43226C` | Action | 1 | target4.timer = table[target4.key] |
| 0x165 | 357 | `0x4322C0` | Action | 2 | target6.timer = table[target6.key] |
| 0x166 | 358 | `0x432078` | Action | 1 | target3.timer = table[target3.key] |
| 0x167 | 359 | `0x432218` | Action | 1 | target2.timer = table[target2.key] |
| 0x168 | 360 | `0x4320CC` | Action | 1 | target5.timer = table[target5.key] |
| 0x169 | 361 | `0x432120` | Action | 1 | for i in 0..base1: target[base0+7+i].timer = table[key] |
| 0x16A | 362 | `0x434880` | Both | 2 | repair/cap party equipment+inventory durability using NPC smithing: threshold `smithing*10`, excess reduced by 30%, current=max |
| 0x16B | 363 | `0x433C94` | Action | 25 | remove target NPC from world, compact group |
| 0x16C | 364 | `0x432928` | Action | 4 | set protagonist +0xD2 = 0xFF |
| 0x16D | 365 | `0x434310` | Action | 18 | `flags_4b2 &= 0xEA` (clears 0x01/0x04/0x10/0x15), zero +0x14/+0x10 (peaceful conquest) |
| 0x16E | 366 | `0x4341B8` | Predicate | 2 | protagonist[+0x56]>>16 > 60 |

Most frequent overall: `0x12E` (353), `0x115` (118), `0x12D` (109), `0x15E`/`0x161`
(47), `0x15D` (45), `0x112` (45), `0x123` (44).

## 4. `B7` flag slots

Block `B7` of `QUEST_R.RES` is **300 four-byte slots** at `0x615068`:

```text
+0x00 u8   flags      (bit 0x80 = "active/set")
+0x01 u8   unknown
+0x02 u16  journal text line (index into B6)
```

`FUN_00434BB8` sets/clears the `0x80` bit, `FUN_00434FE8` reads it (with the
`0x8000` negation flag). The quest-log renderer `FUN_0042CBD0` case 2 prints a
slot when `flags & 0x80` and its `+0x02` line != `0xFFFF`. Slots are initialised
from the file and mutated at runtime.

## 5. Trade / loot

**`0x113`** (`FUN_00431E2C`) is the only opcode that opens an inventory exchange:

1. `_DAT_0081D694 = [player + 0x34]` (money); scans the player inventory
   (`+0x70`) and adds 10 to `_DAT_0081D694` per item of class `0xB`, subtype
   `0x20`.
2. Copies the player inventory (`[player + 0x72]`, `0x40` bytes) into UI row 0
   `0x81CB24`.
3. Resolves the partner role with `FUN_0041182C(_DAT_0081D574)`:
   - no owned waypoint (role 0) or roles 1/5: partner money `[partner + 0x34]`,
     partner inventory `[partner + 0x72]` -> row 2;
   - roles 2/3/4: settlement stock lists `+0x412` (24), `+0x444` (32),
     `+0x484` (23), filtered by `+0x4B2 & 4` and item class > 2;
     partner money = `99999`.
4. `FUN_00430D6C(0)` opens exchange **interface mode 7**.

The trade grid is four 32-slot rows: player bag/offer and partner bag/offer.
Buy, sell, equal exchange and exchange-with-additional-pay are all actions in
mode 7; there are no separate opcodes. Full details, prices, commit rules and
the interface tables are in [`TRADE_SYSTEM.md`](TRADE_SYSTEM.md).

Related but distinct: `0x124` only *grants* a quest item; `0x123`/`0x14C`/`0x162`/
`0x16B` manage world/party actors; `0x158`/`0x16C` only set the interface mode
byte `+0xD2`.

## 6. Execution model / globals

| symbol | role |
|---|---|
| `_DAT_0081D56C` | protagonist `Entity` |
| `_DAT_0081D574` | current dialogue / target `Entity` |
| `_DAT_0081D580` | current settlement record (or 0) |
| `_DAT_0081D55C` | current group-table entry |
| `0x615068` (`B7`) | quest flag slots |
| `0x788FB4` / `0x788DB4` | dialog option lists |
| `0x458A06` | settlement requirement table |
| `0x659B0C` | item-instance table (class/subtype at `+0`) |
| `settlement +0x10` | money pool (u32; `0x11B/0x137/0x16D`) |
| `settlement +0x14` | progress bits (`0x112/0x16D`) |
| `settlement +0x4B2` | runtime flags: `0x10` = independent/not-yet-conquered (clear = князь), `0x20` = merchant loan flag, `0x15` = alert/visited mask |
| `settlement +0x18 + 8n` | target record `{key,state,prop,timer}`; `state == 3` predicates use n = 1,4,6,3,2,5 for ops `0x13B,0x13C,0x13D,0x140,0x142,0x143` |
| `0x458A02 + state*0x40` | u16 requirement (max building skill) |
| `0x458A06 + state*0x40` | u16 value written to a target timer |

Entity fields used by the opcodes: `+0x34` money, `+0x38` free XP, `+0x3C`
invested XP, `+0x40` melee, `+0x42` trade, `+0x44` archery, `+0x46` building,
`+0x48` healing, `+0x4A` identification, `+0x4C` smithing, `+0x4E` base armor,
`+0x56`/`+0x5A`/`+0x30` stats, `+0x64` equipment, `+0x70` inventory, `+0x04`
archetype/flags, `+0xD2` interface mode, `+0xD3` action/state.

## 7. Remake implementation status

All opcodes in the shipped range are dispatched by
`web/src/script.rs::low_family` (parameterized low families) and
`web/src/app/dialogue_ops.rs::QuestContext::exec_op` (high `0x112..0x16E`).
`OPS` remains the machine-readable high catalogue; `op_supported` returns true
for every `0x00..=0x16E`.

| Range | Status | Notes |
|---|---|---|
| `0x00..0x37` | implemented (best-effort) | group active/member predicate derived from `GameTables.groups` + party |
| `0x38..0x6F` | implemented (best-effort) | group has no live member |
| `0x70..0xA5` | implemented | remove inventory item class `0x0B`, subtype `N-1` |
| `0xA6..0xDB` | implemented | add inventory item class `0x0B`, subtype `N-1`; drop at player cell if full |
| `0xDC..0x111` | implemented | inventory contains class `0x0B`, subtype `N-1` |
| `0x112..0x169` | implemented | gold/XP/money/items/party/settlement/target fields |
| `0x16A` | implemented | party gear durability repair/cap (`npc.smithing*10`, 0.7 excess) |
| `0x16B..0x16E` | implemented | remove world NPC, interface bytes, settlement reset, charisma check |

Execution model: `app/dialogue_exec.rs` builds a `QuestContext` snapshot per page
or option selection, evaluates `B4` with `evaluate_condition` and runs `B5` with
`run_action_program`. `Both` opcodes therefore execute during condition
evaluation as in the original. The snapshot is applied back once (and population
is invalidated when actor/settlement state changed).

### 7.1 Save/load

Quest/opcode state added after v1 is stored in `save.rs::SaveExtra`, serialized
after the unchanged `SaveGame` (`SaveEnvelope`, version 2):

- player `Entity+0xD0` dialogue id, `+0xD2` interface mode, `+0xD3` state;
- per-settlement `+0x10` money pool, target base bytes `+0x00/+0x01`;
- per-group `+0x1D` activity byte and visited flag.

The 300 `B7` slots, item-instance table, groups/entities, settlements,
settlement trade stock, party, world markers, projectiles and RNG were already
persisted. Version-1 payloads decode with default extras (migration path).
`app::tests::save` covers v2 round-trip and v1 migration.

## 8. Remaining unknowns / approximations

1. The low group families (`0x00..0x6F`) use the parsed `GameTables.groups`
   and search by location id + object class, matching the original
   `FUN_00432F8C`/`FUN_00433FF8` runtime scan.
2. `0x150` is a pure apprentice-availability predicate (settlement flag 0x10,
   money>=100, visual<3, no role/home waypoint); it never activates a group.
3. `0x162` marks the apprentice (role bits + training timestamp); `0x123`
   applies the role gain when the apprentice later joins. All three role
   branches are implemented: role 1 healing (`+0x48`, clamp 500), role 2 trade
   (`+0x42`, capped by the merchant and 100) and role 3 smithing (`+0x4C`), with
   `gain = (elapsed/16)/0x5A0` and the `0x4505C8` factor `0.03` (`FUN_0043eaae`
   is a plain round helper).
4. `0x159..0x15C` station a **party** specialist via `FUN_004313B0` +
   `FUN_00431654`; `0x14C` settles the dialogue NPC. The four waypoint stations
   are healer (slot 1), merchant (slot 2), smith (slot 3) and voivode (slot 4).
   The stationing copies waypoint `+1` (the role dialogue id) to `Entity+0xD0`
   and clears the apprentice role bits. The producer words are the waypoint owners themselves
   (`Settlement::producer(slot)`), so `0x130` (`+0x3F2`) is the voivode and the
   `FUN_00417F84` barracks training runs for every settlement that has one (see
   [`SETTLEMENT_TRAINING.md`](SETTLEMENT_TRAINING.md)).
5. `B7` slot byte `+0x01` remains unrecovered.
6. `0x16A`/`0x16D` handler addresses are interior labels in Ghidra; their
   current implementations are verified only against the recovered effects.
7. The `FUN_0041D048` settlement block is implemented in
   `app/settlement_pass.rs`: `0x163..0x169` write a raw target timer, then the
   target pass decrements it once per 16 frames; state 0→1/1→2 and 6→0 set
   `local_30`, which skips that settlement's `FUN_00417F84` training pass. The
   same block also runs the `FUN_00416670` waypoint claim, the
   `FUN_0041B658`-power defeat (`>= 2.0`) and the `+0x14` money-pool event. See
   [`SETTLEMENT_TRAINING.md`](SETTLEMENT_TRAINING.md) §4.

## 9. References

- [`DIALOG_SYSTEM.md`](DIALOG_SYSTEM.md) §7 — opcode dispatch + trade.
- [`KONUNG_SOURCE_OF_TRUTH.md`](KONUNG_SOURCE_OF_TRUTH.md) §30 — dialogue/quest.
- [`web/src/script.rs`](../src/script.rs) — machine-readable catalogue.
- [`web/src/quest.rs`](../src/quest.rs) — `QUEST_R.RES` decoder (programs, text, flags).
