# Konung multiplayer client — screen state machine

Target: `CLIENT.EXE` (PE32/i386, base `0x400000`,
`sha256 adf0a2793ee42b16dcf31c53799cf5c9b0ca37471873e1888c26e2b5f01bf4ac`).
All addresses are `CLIENT.EXE`-specific.

This is the behavioural model of the shipped multiplayer client: the screen
states, their per-frame handlers, transitions, and the input routing that feeds
them. It is derived from the state jump table at `0x429F44`, the index tables of
each state handler, and a local unattended run that displayed the 640×480 main
menu (`Игра / Новый герой / Установки / Выход`).

## 1. State variables

| variable | address | domain | meaning |
|---|---|---|---|
| `_DAT_006EB774` | `0x6EB774` | **screen state** | `0..0xB`; selects the per-frame handler |
| `_DAT_006EB7AC` | `0x6EB7AC` | **pending action** | state/action code committed by the arbiter state 0 and by state handlers |
| `_DAT_006EB7BC` | `0x6EB7BC` | **venue busy count** | non-zero blocks the state-0 transition |
| `_DAT_006EB764` | `0x6EB764` | **session flag** | gates the special state-0 → `0xA` transition |
| `_DAT_00757800` | `0x757800` | **interface mode** | `0` world, `5` map/travel, `7` inventory detail (same shape as the single-player build) |
| `_DAT_0075780C` | `0x75780C` | **modal flags** | bit 0 input block, bits 1/2/3 interface-open requests |
| `_DAT_006EB780` | `0x6EB780` | **quit request** | consumed by WndProc |
| `_DAT_007577E4` | `0x7577E4` | **player/host role** | `0` client, non-zero host (gates several menu branches) |
| `_DAT_006EB358` | `0x6EB358` | **error flag** | set by the queue pump; triggers the fatal path |
| `_DAT_00757848` | `0x757848` | **map/location id** | used by the map start and character screens |

The screen state is initialised to `9` in WinMain and immediately becomes `3`
(the main menu) through the state-9 preload (see §4.9).

## 2. State dispatch

`FUN_0042855E` (`0x42855E`, 6 407 bytes, non-contiguous) is the per-frame input
and state driver. It runs inside the `WM_USER 0x400` frame:

```text
FUN_0042D094():                 # network/queue pump (0x42D094)
    _DAT_006EB7C8++             # frame counter
    _DAT_006EB358 = 0
    FUN_0042855E()              # consume one input event + state handler
    drain <=15 network packets  # FUN_0042D574 / FUN_0042CA4F on 0x6EB154
    FUN_0042A00B()              # venue keepalive / "Server failed" gate
    if (_DAT_006EB358) FUN_0042D620()
FUN_0042E19C():                 # frame compositor/present (0x42E19C)
```

`FUN_0042855E` structure:

1. Consume **exactly one** keyboard-ring event (`values[16] @ 0x6EAE64`,
   `flags[16] @ 0x6EB0D0`, head `0x6EB798`, tail `0x6EB794`).
2. Update Shift/Ctrl modifier levels; ESC is handled here (see §5).
3. Latch mouse button edges into `_DAT_007577B0`; integrate the mouse delta
   (`FUN_0041EC52`) into the clamped cursor `_DAT_007577BC / _DAT_007577C0`.
4. Dispatch on `_DAT_006EB774` through the table at **`0x429F44`**:

| state | handler | table slot |
|---|---|---|
| `0` | `0x4288B5` | `0x429F44` |
| `1` | `0x4288EB` | `0x429F48` |
| `2` | `0x428EBC` | `0x429F4C` |
| `3` | `0x428A5E` | `0x429F50` |
| `4` | `0x428F02` | `0x429F54` |
| `5` | `0x429769` | `0x429F58` |
| `6` | `0x429AC9` | `0x429F5C` |
| `7` | `0x429F3F` | `0x429F60` |
| `8` | `0x429ADE` | `0x429F64` |
| `9` | `0x429D6E` | `0x429F68` |
| `10` (`0xA`) | `0x429E0B` | `0x429F6C` |
| `11` (`0xB`) | `0x429EEB` | `0x429F70` |

Bounds check: `state > 0xB` falls through to `0x429F3F` (no-op). Evidence for
state reads: `ev_4d476219863d929e6d9d558110be577b67ca508e325a9f2b8ca3e5497e18bddf`.

## 3. State table

| id | name (confidence) | handler | evidence |
|---|---|---|---|
| `0` | transition arbiter | `0x4288B5` | reads `0x6EB7AC`/`0x6EB7BC`/`0x6EB764` |
| `1` | **gameplay** (verified) | `0x4288EB` | WM_PAINT draws only when state `1`; world-update call list |
| `2` | **team/player scoreboard** (verified) | `0x428EBC` | draws 10 slots via `FUN_0042255A` |
| `3` | **main menu** (verified by screenshot) | `0x428A5E` | items `Игра / Новый герой / Установки / Выход` |
| `4` | **character / new-hero editor** (inferred) | `0x428F02` | 33-entry index table, paper-doll render, stat fields |
| `5` | **session / server screen** (inferred) | `0x429769` | DirectPlay join, server list, map start |
| `6` | **fatal error** (verified) | `0x429AC9` | calls `FUN_004169DA` (message box/log) |
| `7` | no-op | `0x429F3F` | empty |
| `8` | **settings** (verified) | `0x429ADE` | 10-entry table, six option flags, video re-init |
| `9` | **initial preload/transition** (verified) | `0x429D6E` | sets state `3`; loads `ME<w>R.RES` |
| `10` (`0xA`) | **start gameplay** (inferred) | `0x429E0B` | sets state `1`, clears world/input state |
| `11` (`0xB`) | **new-game commit** (inferred) | `0x429EEB` | copies 0x100-byte hero template, map start |

## 4. Per-state handlers

### 4.1 State 0 — transition arbiter (`0x4288B5`, 54 bytes)

```text
if (_DAT_006EB7AC == 0xA && _DAT_006EB764 != 0) {
    _DAT_006EB774 = 0xA;
} else if (_DAT_006EB7BC == 0) {
    _DAT_006EB774 = _DAT_006EB7AC;      // commit the pending action
}
```

This is the only state that copies a pending action into the screen state, so
`0x6EB7AC` is the client's equivalent of a "next screen" code.

### 4.2 State 1 — gameplay (`0x4288EB`)

Calls (direct, in order): `0x432945` (rand), `0x42DE5C` (sound), `0x41BBA5`,
`0x41B7DF` (sprite blit), `0x4144BD`, `0x4147DF`, `0x411ECE`, `0x413479`
(world/actor passes), `0x4183A5`, `0x4184D8` (active lists), `0x42289F`,
`0x425812`, `0x427C52` (quest/interface), `0x414E11`, `0x41EE12`.

Globals read: `0x6EB7B0` (map id), `0x757800` (interface mode), `0x75780C`
(modal flags), `0x6EB760`, `0x757890/0x757894` (effect counters),
`0x7578BC`, `0x7578E4`, `0x7578EC`, `0x757908`, `0x75790C`, `0x6EB788`
(world tick), `0x6F1BEC/0x6F1BE8` (camera offsets).

This matches the single-player world-update shape but uses `CLIENT.EXE`
addresses; the same overlay/actor tables are not yet ported.

### 4.3 State 2 — team/player scoreboard (`0x428EBC`)

```text
if (_DAT_007577AC != 0) {          // any mouse button pressed
    _DAT_006EB774 = 9;             // back to the preload/transition
    _DAT_006EB7AC = 2;
    _DAT_007577E4 = 1;
} else if (_DAT_00757808 != 0) {
    _DAT_00757808 = 0;
    FUN_0042255A();                // draw the 10-slot player list
}
```

`FUN_0042255A` iterates **10 player slots** at `0x5E812C`, stride `0x74`
(116 bytes) — exactly the `0x488` bytes that the main-menu "Новый герой"
branch zero-initialises. Each slot holds a number, a name and several numeric
fields; the renderer uses `FUN_00433865` (itoa) and `FUN_0041B8D7` (text blit).
The install string `Счет команд` (`0x440336`, CP866) confirms the screen
purpose.

### 4.4 State 3 — main menu (`0x428A5E`)

Observed screen (640×480): a stone slab on a forest background with four items
`Игра`, `Новый герой`, `Установки`, `Выход`, highlighted by hover.

```text
idx = FUN_00416F91(cursor, table 0x4491D0);
if (_DAT_007577E4 == 0 && idx < 2) idx = -1;      // role-gated entries
FUN_00422C76(idx);                                 // hover/highlight
if (mouse-left && idx > -1) {
    FUN_0042DE5C(4,0,0,0);                          // click sound
    switch (idx) { ... table 0x428E8C, 6 entries ... }
}
```

Index table `0x428E8C` (6 entries):

| idx | handler | effect |
|---|---|---|
| `0` | `0x428AD1` | `_DAT_006EB774 = 1`; `FUN_004202D2`, `FUN_00420A96(-1)`, `FUN_0041EEE1` — **start gameplay** |
| `1` | `0x428AFE` | `_DAT_006EB774 = 2`; `memset(0x5E812C,0,0x488)`; `FUN_0041FDA7` — **create local player / new hero**, then show the player list |
| `2` | `0x428B90` | character-field setup + `FUN_004284A0` |
| `3` | `0x428BEF` | character-field setup + `FUN_00411FED`, `FUN_00422E07` |
| `4` | `0x428DB9` | copies the six option flags `0x7577CC..0x757818` from `0x7784AC..`; `FUN_004250A2` — **options** |
| `5` | `0x428E1C` | `FUN_004284A0` only — **exit/cancel** candidate |

The visible item→index order is **not** established: the hit-test table
`0x4491D0` has 29 records whose record size/coordinate space is unresolved, so
the mapping from the four drawn labels to indices `0..5` remains **UNKNOWN**.

### 4.5 State 4 — character / new-hero editor (`0x428F02`)

- Hit-test table `0x44D790`; index switch at `0x429730` with a **33-entry**
  table at `0x4296AC` (indices `0..0x20`).
- 24 of the 33 slots are live; the rest fall to `0x4296A7` (no-op).
- Case bodies call `FUN_0042F4AE` (paper-doll/character render), `FUN_00432726`
  (rect fill), `FUN_004329F4`/`FUN_00432A19` (string copy/concat) and
  `FUN_00422E07`.
- Fields touched cluster in `0x75759A..0x75764B` (stat/equipment/portrait
  controls) and `0x757844/0x757848` (map id).
- This is the multiplayer equivalent of the single-player New-Hero screen; the
  exact control labels are **UNKNOWN** (image-based UI).

### 4.6 State 5 — session / server screen (`0x429769`)

- Hit-test table `0x44DE10`; the index chain at `0x429A27` covers
  `0..0x12` (18 ranges).
- Calls include `FUN_0042D1EC` (DirectPlay enumerate/lobby/join),
  `FUN_0042467C` (map/session start), `FUN_0041701E` (server-list load),
  `FUN_0042D174`, `FUN_0042D620`.
- Case `0x4297A8` writes `0x7575BD = 0x7C`, `0x7575BF = 1`, looks up
  `0x451810[map_id]`, stores it at `0x7575C1`, then calls `FUN_0042467C(0)`.
- Role: connect/host selection and session start. Labels are **UNKNOWN**.

### 4.7 State 6 — fatal (`0x429AC9`)

```text
FUN_004169DA(<message>, 0x10);   // log to CLIENT.LOG + message box
```

A 21-byte state that only shows/logs an error (the reachable message is
`Server failed` at `0x44060B`, but the pushed address is the byte before it, so
the exact text depends on the caller).

### 4.8 State 7 — no-op (`0x429F3F`)

Empty handler; falls straight back to the frame epilogue.

### 4.9 State 8 — settings (`0x429ADE`)

- Hit-test table `0x44E250`; index switch at `0x429D48` with a **10-entry**
  table at `0x429D20` (indices `0..9`).
- Cases `0..7` toggle the six option flags `0x7577CC`, `0x7577E0`, `0x7577F0`,
  `0x7577F8`, `0x757810`, `0x757818` and copy them to the working set
  `0x7784AC..0x7784C4`; case `0x429C0F` applies (`FUN_004250A2`), case
  `0x429CCC` is OK/cancel.
- `FUN_0041EB35` (FLAMES.RES loader) and `FUN_0041B97D` (video re-init) are
  called on apply, mirroring the single-player options screen.

### 4.10 State 9 — initial preload/transition (`0x429D6E`)

```text
_DAT_006EB774 = 3;                       // commit: next state is the main menu
_DAT_007577B8 = 0; _DAT_007577B4 = 0;
FUN_0041EF3E();
idx = FUN_00416F91(cursor, 0x4491D0);
if (_DAT_007577E4 == 0 && idx < 2) idx = -1;
if (_DAT_006EB7AC != 8) FUN_0041FF3F();  // load ME<width>R.RES
else                    FUN_00420A96(...); // alternate start path
```

`FUN_0041FF3F` builds `"ME" + itoa(_DAT_007578B0) + "R" + ".RES"`
(`0x4402F4`+`0x4402F7`+`0x4402F9`) — i.e. `ME640R.RES` or `ME800R.RES` — and
loads it into the interface buffers. It is the first archive the client needs;
without it the process logs `"ME640R.RES not found rc=16"` and exits 1.

### 4.11 State 10 — start gameplay (`0x429E0B`)

```text
_DAT_006EB774 = 1;
_DAT_006EB760 = 0; _DAT_006EB784 = 0; _DAT_006EB77C = 0;
_DAT_0075781C = 0; _DAT_00757800 = 0; _DAT_004F5C20 = 0; _DAT_007577EC = 0;
_DAT_007578BC = 0; _DAT_006EB788 = 0;
FUN_004328F0(0x6EAE28, 0, 0x0C);         // clear input/session scratch
...
```

Clears the world/interface/input state and commits state 1 (gameplay).

### 4.12 State 11 — new-game commit (`0x429EEB`)

```text
_DAT_006EB774 = 5;
_DAT_0075781C = 0; _DAT_007577EC = 0; _DAT_007577E8 = 0; _DAT_0075787C = 0;
FUN_0041FDA7();
rep movsd 0x757680 -> 0x757580, 0x40 dwords;   // 0x100-byte hero template
FUN_0042467C(0);                                 // map/session start
```

Copies the 0x100-byte hero template and starts the map, then commits state 5
(session screen). This is the client's "load into world" path.

## 5. ESC handling

ESC is consumed inside `FUN_0042855E` before the state dispatch (same location
as the single-player build). The decoded behaviour:

- state `1`: `FUN_00420A96`/interface handling; the world/interface close path.
- other states: falls through to the state handler, which does not treat ESC
  specially (the menu uses the mouse).

The exact ESC table could not be decoded as a single table; it is folded into
the input prologue of `FUN_0042855E` and remains partially **UNKNOWN**.

## 6. Input routing matrix

| state | keyboard | mouse |
|---|---|---|
| `0` | none (arbiter) | none |
| `1` gameplay | one VK per 60 ms, gated by `0x75780C & 1` | full command tree via `FUN_0042260C`-equivalent |
| `2` scoreboard | none | any button → state 9 |
| `3` main menu | ESC | hit-test `0x4491D0`, 6-entry switch |
| `4` character | ESC | hit-test `0x44D790`, 33-entry switch |
| `5` session | ESC | hit-test `0x44DE10`, 18-range chain |
| `6` fatal | none | none |
| `7` no-op | none | none |
| `8` settings | ESC | hit-test `0x44E250`, 10-entry switch |
| `9` preload | none (one frame) | none |
| `10` start | none (one frame) | none |
| `11` commit | none (one frame) | none |

## 7. Invariants

- Exactly one screen state is active per frame; the handler runs once per
  `WM_USER 0x400`.
- Exactly one keyboard event is consumed per frame.
- Mouse state is sampled; button presses are latched into `0x7577B0` and
  consumed by the current state handler.
- State transitions are committed either directly (`_DAT_006EB774 = n`) or via
  the arbiter state 0 through `_DAT_006EB7AC`.
- Frame budget is 60 ms (`0x6EB778`).

## 8. Residual unknowns

1. Visible menu-item → hit-test index mapping (state 3, table `0x4491D0`).
2. Labels for states 4, 5, 10, 11 (character/session screens) — inferred from
   behaviour, not from a decoded label.
3. ESC table in `FUN_0042855E`.
4. State 2 "scoreboard" is confirmed by the 10-slot renderer and the
   `Счет команд` string, but the on-screen trigger for it is not located.
