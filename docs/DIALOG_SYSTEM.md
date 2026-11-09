# Konung dialogue / quest system — reverse-engineered notes

Target: `konung.res` (the shipped PE32/i386 game executable) + `QUEST_R.RES`.
All addresses are virtual addresses in `konung.res`; block addresses are the
fixed BSS buffers the loader writes into.

## 1. Which file holds dialogue

**`QUEST_R.RES`** (shipped here as `QUESTR.RES` / `Quest_r.res`, 337 800 bytes).
`konung.res` holds only the *code*. The loader `FUN_0043a870` (`0x43A870`) opens
`<base>\QUEST_R.RES` (string `0x450770`) right after `Game.N`, and reads its
payload as seven fixed, contiguous blocks whose sizes sum to the file size:

| Block | File offset | Length | Buffer VA | Role |
|---|---|---|---|---|
| B1 | 0 | 128 000 | `0x5C2D90` | node/option records, stride `0x10` |
| B2 | 128 000 | 600 | `0x5E2190` | `u16[300]`: `dialogue_id -> first node` |
| B3 | 128 600 | 64 000 | `0x5E23E8` | CP866 text blob (NUL-terminated) |
| B4 | 192 600 | 12 000 | `0x5F1DE8` | `u16[]` condition programs (`0xFFFF`-ended) |
| B5 | 204 600 | 12 000 | `0x5F4CC8` | `u16[]` action programs (`0xFFFF`-ended) |
| B6 | 216 600 | 120 000 | `0x5F7BA8` | `u16[]` per-line lists of B3 offsets |
| B7 | 336 600 | 1 200 | `0x615068` | `u32[300]` quest/global flag slots |

If the open fails the original reports `"File QUEST.RES not found"` (`0x45077C`).

## 2. Entry point — right-clicking an NPC

The gameplay driver `FUN_0042260C` (`0x42260C`) selects the clicked actor into
`_DAT_0081D578` and, on a **right-click** (right-button branch, `_DAT_0081D5B0 &
2`, command code 0) when that actor's `Entity+0xD0` (`dialogue_id`) is not
`0xFF`, calls the dialogue opener:

```c
if ((_DAT_0081D578 != 0) && (*(byte *)(_DAT_0081D578 + 0xD0) != 0xFF) &&
    (_DAT_0081D624 == 0) && ((_DAT_0081D56C[3] & 4) == 0)) {
    FUN_00435290(_DAT_0081D578);   // dialog entry
    FUN_0042CBD0(0);               // render dialog interface mode 0
}
```

`FUN_00435290(entity)`:

```c
_DAT_0081D674 = *(int *)(&DAT_005E218E + entity->dialogue_id * 2) >> 16;  // B2[id] -> node
_DAT_0081D574 = entity;                                                   // NPC pointer
_DAT_00831C4C = _DAT_0081D674;
if ((entity->archetype & 0x40) == 0) { window = rng()%4 + 0x0C; }        // humanoid
else if (archetype == 'Q') window = 0x11;
else if (archetype == 'R') window = 0x12;
else if (archetype == 'S') window = 0x13;
else window = 0x10;
if (FUN_00434C74()) FUN_0042A13C(window, 0, 0, 0);                       // open window
```

So `dialogue_id` indexes **B2** for the first node, and the NPC archetype
(`Entity+0x04`) + resolution selects the window template id (`0x0C..0x13`).

## 3. B1 record (`0x10` bytes)

Two shapes share the 16-byte slot (`FUN_00434C74`):

- **Page header** (`+0x00 = +0x02 = 0xFFFF`):
  `+0x04` next page, `+0x0A` `0xFFFF` = display page / `0xFFFE` = branch page,
  `+0x0C`(low) = *unconditional* entry count, `+0x0E`(high) = *conditional* entry
  count. The decompiler reads the entry count as `(u32 @ +0x0C) >> 16` for the
  first (conditional) pass and the low word for the second pass, so a **display
  page's entries** are the `conditional + unconditional` records starting at
  `header+1`; a **branch page's cases** are the `conditional` records starting at
  the header itself.
- **Entry / option**: `+0x00` u32 = **B6 line index** (`0xFFFFFFFF` = none),
  `+0x04` next node (`0xFFFF` = fall back to the page, `0xFFFE` = end dialogue),
  `+0x06` B4 condition program id (`0xFFFF` = unconditional), `+0x0A` B5 action
  program id (`0xFFFF` = none).

Selection is handled in `FUN_0042260C` case `0x65`: it runs the option's action
program (`FUN_00434BB8`), resolves the next node from the option's `+0x04`
(`-1` -> fall back, `-2` -> end), rebuilds the option lists (`FUN_00434C74`) and
redraws (`FUN_0042CBD0(0)`).

**Verified against the shipped file:** `Game.1` entity 277 (Гилли) has
`dialogue_id = 140` -> node 3763 (branch) -> page 3765 whose first entry's B6
line 28556 composes to the Черный Бор elder greeting *"Приветствую тебя, о
Великий Князь! …"*.

## 4. Text (B3 / B6)

`FUN_00435188` composes a line from a **B6** list: each `u16` word is a **B3**
byte offset, terminated by `0xFFFF`; fragments are decoded as CP866. A single
space (the separator string at `0x450618`) is inserted before a following
fragment whose first byte is a letter (`> 0x40`), `-`, or digit (`0x30..0x39`),
matching the original exactly. `B3` text is therefore an offset pool, not one
string; word boundaries are recovered by the separator rule.

## 5. Programs (B4 / B5) and flags (B7)

A program is a `u16` list ended by `0xFFFF`. Each word: low 9 bits = operand,
`0x2000` = run original script opcode `FUN_0043159C(operand)`, `0x4000` =
condition group separator (used only by the condition evaluator
`FUN_00434FE8`), `0x8000` = set/negate the B7 flag slot `operand`.

- **Condition `FUN_00434FE8(id)`**: OR-of-ANDs over B4; a term is either a B7
  flag bit `0x80` or an opcode predicate; `0x8000` negates a term.
- **Action `FUN_00434BB8(id)`**: for each word, either set/clear B7 flag
  `operand` bit `0x80` (per `0x8000`) or call `FUN_0043159C(operand)`.
- **Opcode dispatcher `FUN_0043159C(op)`**: `op < 0x38` -> `FUN_00432F8C(op+1)`;
  `< 0x70` -> `FUN_00433FF8(op-0x37)`; `< 0xA6` -> `FUN_0043395C(op-0x6F)`;
  `< 0xDC` -> `FUN_004347D0(op-0xA5)` (spawns an entity/item, kind `0x0B`);
  `< 0x112` -> `FUN_00433A8C(op-0xDB)`; else a table at `0x465BD4`
  (**unresolved** — entries decode as small integers, not code pointers).

## 6. Interface (INTR) — scope 1

The dialogue interface is drawn by **`FUN_0042CBD0(param = 0)`** (switch case
`param+1 == 1`; it sets interface mode `_DAT_0081D610 = 1`). This is the same
`FUN_0042CBD0` dispatcher as the character screen (case 3) and the quest log
(case 2) / inventory (case 5); `FUN_0042CBD0(-1)` (case 0) restores the normal
HUD (`INTR[3]` + party portraits).

Case 1 composes the window from three INTR sprites at per-resolution table
positions (`_DAT_0081D644`: 0 = 640, 1 = 800):

1. **Stone background (tiled).** `FUN_00430430` (gated by `_DAT_0081D614`) tiles
   **`INTR[5]`** — a full-opaque **100x100** tile (`0x61F400` = entry[5]) — in
   100-px steps (`FUN_00427698`, clipped to the viewport rect
   `0x81D738/0x81D754/0x81D744/0x81D760`). This is the visible stone fill.
2. **Ornate border.** **`INTR[1]`** (`0x61F3E0` = entry[1]; 651x470@800, only
   ~3% opaque — a frame with a transparent centre) at table `0x4632C4/0x4632C8`
   (640: (114,0); 800: (144,0)).
3. **Divider band.** **`INTR[0]`** (`0x61F3D8` = entry[0]; 615x55@800, ~10%
   opaque) at table `0x4632D4/0x4632D8` (640: (129,150); 800: (161,187)).

NPC text and options are drawn with the small font (`0x58BC34`); the **selected**
option uses the highlight font (`0x58B8F0`, a same-metric small variant). Text is
laid out by `FUN_0042BD8C`, which wraps to a max width and advances **15 (0xF) px**
per line. Option/text column tables: prompt column at `0x4632E4/0x4632E8`
(width `0x463304`), option column at `0x4632F4/0x4632F8`.

The two runtime lists are built by `FUN_00434C74`: `0x788FB4` (the `f7`
*conditional* page entries = NPC statement lines) and `0x788DB4` (the `f6`
*unconditional* entries = selectable options, count `_DAT_0081D6D0`). Each entry
stores its `B6` text-line index at `+8`. The current option is highlighted from
input command `100` (`index = value >> 8`); command `101` selects it
(`FUN_0042260C` case `0x65`).

> Correction: `FUN_0042A13C` is **not** a window opener — it is the **sound
> playback** routine (the first archive `FUN_004392B4` loads, filename literal
> `SOUNDS.RES`, contains PCM blocks). The `0x0C..0x13` value passed by
> `FUN_00435290` is an NPC **voice line id**, not a window id.

## 7. Script opcodes `0x112..0x16E` (actions / conditions)

> **Implementation status (2026-10-22):** every low family `0x00..0x111` and
> every high opcode `0x112..0x16E` is dispatched. The executable handlers live
> in `web/src/app/dialogue_ops.rs` (`QuestContext::exec_op`); `0x16A` is
> implemented as party gear durability repair/cap (`npc.smithing*10`, 30%
> excess reduction). The
> machine-readable high catalogue is still `web/src/script.rs::OPS`, with
> `low_family`/`op_supported` for the parameterized ranges. The quest additions
> (protagonist `+0xD0/+0xD2/+0xD3`, settlement money pool/target base bytes,
> group activity/visited) are persisted in `save.rs::SaveExtra` (save v2; v1
> migrates with defaults). See `QUEST_OPCODES.md` ?7 for the full status table.


For `op >= 0x112`, `FUN_0043159C` does `CALL [op*4 + 0x465BD4]` — an **op-indexed**
function-pointer table; entries below `0x112` are unused filler, so the real table
begins at `0x46601C`. Shipped dialogues use `op 0x112..0x16E` (93 handlers),
catalogued with handler VA, kind (predicate/action) and effect in
[`web/src/script.rs`](web/src/script.rs) (const `OPS`); the full human-readable
reference (including the low-opcode families `0x00..0x111`) is
[`QUEST_OPCODES.md`](QUEST_OPCODES.md).

Highlights: `0x115` = `rng()%100<50`; `0x11D..0x12A` = gold −/+ families
(5/10/20/50/100/250); `0x12B..0x12D` = grant 10/20/50 XP; `0x131..0x136` = money
predicates; `0x13B/0x13C/0x13D/0x140/0x142/0x143` = settlement quest-state == 3;
`0x139/0x13E/0x149` = owns item class 9 subtype N; `0x148/0x151..0x157/0x166` =
settlement requirement table `0x458A06`; `0x123/0x14C/0x162/0x16B` = spawn/
recruit/remove world actors. `0x14C` settles a party NPC into the current
settlement (and copies waypoint 0's `+1` role dialogue id onto them); `0x159`–
`0x15C` station a party specialist at waypoint 4/1/3/2 (voivode/healer/smith/
merchant), copying the waypoint `+1` dialogue id and clearing the apprentice role
bits. `0x16D` clears `Settlement.flags_4b2 &= 0xEA` (independent/hostile/alert
bits) and zeroes progress/money; this is the peaceful-conquest transfer.
`0x11B` pays the settlement money pool; `0x137` tests it non-zero. Full village
trace: `SETTLEMENT_SYSTEM.md`.

**Trade/loot = `0x113` (`FUN_00431E2C`), confirmed.** It loads the player's
inventory (`0x81CB24`) + money (`_DAT_0081D694`) and the partner's inventory
(`0x81CBA4`) + money (`_DAT_0081D698`), resolves the partner role via
`FUN_0041182C(_DAT_0081D574)`, then opens exchange **interface mode 7** with
`FUN_00430D6C(0)`. No other op opens an inventory exchange; `0x124` only grants
an item. Implemented in `trade.rs` / `trade_screen.rs` / `app/trade.rs`; see
[`TRADE_SYSTEM.md`](TRADE_SYSTEM.md).

## 8. Remaining unknowns

1. `B1` `+0x06` on display headers; `B7` slot byte `+0x01`.
2. The NPC **voice line ids** (`0x0C..0x13` per archetype) beyond "a sound is
   played" (the clips are in `SOUNDS.RES`; see `SOUND_SYSTEM.md`).
3. Per-op argument semantics where a handler reads no pushed argument.

## 9. Remake integration

`web/src/quest.rs` decodes the database (`QuestData::parse`, `dialogue_start`,
`node`/`option`, `entries`, `line_text`, `condition_program`/`action_program`,
`flag_byte`). `web/src/dialog.rs` implements the interface:

- **Entry:** a **right-click** that hits a **human** actor (`archetype & 0x40 ==
  0`) with `dialogue_id != 0xFF` opens `DialogState` (from `dialogue_start`).
  Left-click only moves; a right-click on any other actor (monster / non-talkable
  human) falls through to a move (the original orders an attack there).
- **Navigation:** the page's `conditional` (f7) entries are the NPC statement
  lines and the `unconditional` (f6) entries are the options (`-2`/`-1` links end
  the dialog). Hover highlights; click selects and follows the option's `+0x04`.
- **Render:** tiled `INTR[5]` stone background (100x100, clipped to the window
  rect) then `INTR[1]` border + `INTR[0]` divider at the decoded per-resolution
  positions, text with the original small font (`0x58BC34`), selected option with
  `0x58B8F0`.
- **Actions/conditions are executed** (`web/src/dialog.rs` + `web/src/script.rs` +
  `web/src/app/dialogue_exec.rs`): `FUN_00434C74` semantics are reproduced — the
  `conditional` (f7) entries are the statement lines and the `unconditional` (f6)
  entries the selectable options; an entry is included only when its `B4`
  condition program (evaluated by `FUN_00434FE8`) is absent or true; branch pages
  (`+0x0A == -2`) select the first case whose condition holds. A passing
  statement's `B5` action runs on page build (`FUN_00434BB8`) and the selected
  option's action runs before following its `+0x04`. The full `0x112..0x16E`
  opcode catalogue is in `web/src/script.rs`; trade/loot (`0x113`) opens the
  exchange interface (`trade.rs` / `trade_screen.rs` / `app/trade.rs`).

- **Two quest files.** The executable opens the literal `QUEST_R.RES`. The install
  ships it as `Quest_r.res` (case-insensitive alias of `QUEST_R.RES`; the real
  script) and a *different* placeholder `QUESTR.RES` whose dialogues 97/96 are
  literal `"???"` stubs. `web/tools/make_manifest.py` prefers `QUEST_R.RES`, then
  `Quest_r.res`, then `QUESTR.RES`.

- **Party recruitment.** Opcode `0x123` (`FUN_00431820`) appends the dialogue NPC
  to the reserved party **group 0** (`+0x05=0`, `+0x02=0x60`, `+0x14=0xFF`);
  `0x14C` (`FUN_00432954`) dismisses it. The party-member-vs-not dialogue branches
  are driven by `0x15E` (`Entity+5==0`), `0x161` (`Entity+4 & 0x38`),
  `0x15D` (`max_party <= count`) and `0x160` (level>3). Example: Сигмар (`Game.1`
  entity 539, `dialogue_id 97`) — branch node 1767 → first-meeting page 1771 →
  node 1774 → `0x123`; once a member, branch node 1781 (`0x14C` to dismiss).
  Recruited companions leave their world group (the location population filters
  party ids), render in the world depth pass and **follow the leader**
  (`web/src/app/party.rs`: `FUN_00421b50` group-0 inline AI → `FUN_0041ecc8` →
  `FUN_0041f3c8`; re-target a free cell beside the group's first entity only when
  `FUN_0043881c` distance `>= 10`, one cell per walk cycle).

The browser loads `QUEST_R.RES` as `KIND_QUEST = 13` (`manifest.quest`); the ABI
exposes `debug_quest_dialogue_count` and the `debug_trade_*` helpers. Exact
voice-line playback remains a follow-up.
