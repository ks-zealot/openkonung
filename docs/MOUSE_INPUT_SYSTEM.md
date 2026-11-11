# Konung mouse / input dispatch — reverse-engineered notes

Target: `konung.res` (shipped PE32/i386, image base `0x400000`,
`sha256 d698c4f231489a372cc25212ab2819720a36d7ce5307a045db684135cf679810`).
All addresses are virtual addresses in `konung.res` unless stated otherwise.

This document describes the **whole mouse/input pipeline**: how a Win32 mouse
message becomes a per-frame game command, every gate that can intercept it, and
exactly what each click does in the world (empty/passable cell, blocked cell,
NPC, mob, item). It is derived with the REA Ghidra provider (12.1.4) plus a
Capstone disassembly of the un-analysed input region, and cross-checked against
the remake (`web/src/app/input.rs`, `web/src/dialog.rs`).

> Evidence anchors used below: `analyze_function` dossiers retained as
> `ev_ea8fe3cf…` (`FUN_0042260C`), `ev_f7409e0b…` (`FUN_004381C8`),
> `ev_987d36f8…` (`FUN_004142D4`); `FUN_00436214` pseudocode in
> `KONUNG_GAME2_NPC_TIMER_REA_EVIDENCE_2026-10-13.json` (`ev_5e6e4b0a…`).

---

## 0. Pipeline overview

```
        Windows USER32 message queue
                 │  (class "Konung", registered in FUN_0042B1A8)
                 ▼
 (A)  WndProc  FUN_0042B65C  ─────────────►  input state globals
        WM_MOUSEMOVE / WM_LBUTTONDOWN/UP / WM_RBUTTONDOWN/UP
        WM_KEYDOWN / WM_KEYUP / WM_ACTIVATEAPP / WM_SETCURSOR / …
                 │                                    │ button bits  _81D5B0
                 │ mouse lParam x/y                   │ accum. delta _81D5B8/_81D5BC
                 ▼                                    │ ring events  _81C154/_81CF70
        per frame:  FUN_00428C00  (integrate delta -> _81D5C0/_81D5C4, clamp)
                 │
        frame tick  WM_USER 0x400  (main loop, 60 ms budget _81D5C8 = 0x3C)
                 ▼
 (B)  FUN_00436214  ── pops 1 ring event, handles keys/ESC, button edges,
        dispatches the current screen (_81D5AC) and, in gameplay, calls →
                 ▼
 (C)  FUN_0042260C  ── the UI/click dispatcher.
        • reads the current mouse command from  FUN_004381C8   (command compiler)
        • left button  (_81D5B0 & 1):  UI commands + "move party" (code 0)
        • right button (_81D5B0 & 2):  world order/interaction (talk/attack/move)
        • no button:                   hover tooltips + hovered-actor name
                 ▼
        world click  →  screen->cell (FUN_00438AB4) → destination stored on the
        selected units (Entity+0x0C/+0x0E) → route built later over the collision
        grid (FUN_0043DA1F) → walk (FUN_0041435C); blocked ⇒ no route ⇒ no move.
```

The key top-level state:
- `_DAT_0081D5AC` — application/screen state (0 = in-game; menu/screens otherwise).
  Set to `-1` at startup (`0x42B1B4`), `9` by init (`0x42B2B0`), and driven by the
  ESC switch in `FUN_00436214`.
- `_DAT_0081D610` — **interface mode** (0 world, 1 dialogue, 2 quest-log, 3
  character, 5 world map/travel, 7 inventory-detail). This is the main gate used
  by the command compiler `FUN_004381C8`.
- `_DAT_0081D5B0` — live mouse-button bits: **bit 0 = left, bit 1 = right**.
- `_DAT_0081D5C0` / `_DAT_0081D5C4` — the in-game (clamped) cursor x/y.

---

## 1. Stage A — Win32 message layer

### 1.1 Window / class / main loop

- Window class name string `"Konung"` at `0x4632BC`; used as both class name and
  window title. Class is registered from `FUN_0042B1A8` (a WNDCLASS built on its
  stack with `lpfnWndProc = 0x42B65C`, `hIcon = "GAME_ICON"`), and the window is
  created with `CreateWindowExA` at `0x42B25F` (class `0x4632BC`). Handle stored
  at `_DAT_0082D230`.
- Imports used (from `.idata`): `RegisterClassA` (`0x84018C`), `CreateWindowExA`
  (`0x840160`), `DefWindowProcA` (`0x840164`), `GetMessageA` (`0x840174`),
  `PeekMessageA` (`0x840184`), `TranslateMessage` (`0x8401A4`),
  `DispatchMessageA` (`0x84016C`), `PostQuitMessage` (`0x840188`), `BeginPaint`
  (`0x84015C`), `EndPaint` (`0x840170`), `SetCursor` (`0x840194`),
  `ShowCursor` (`0x84019C`), `timeGetTime` (`0x8401C0`). **No DirectInput** — the
  mouse is fed from ordinary window messages.
- Main loop (around `0x42B58C`–`0x42B642`):
  - when `_DAT_0081D5F4 != 0` → `GetMessageA(&msg,0,0,0)` then
    `TranslateMessage`/`DispatchMessageA`;
  - the frame is produced by `timeGetTime()` vs `_DAT_0081D5C8` (the **60 ms**
    budget `0x3C`, initialised at `0x42B32F`); when elapsed it calls
    `SendMessageA(hwnd, 0x400, 0, 0)` (`0x42B635`).
- `_DAT_0082D2C8` = "app active" (set from WM_ACTIVATEAPP), `_DAT_0082D2C9` =
  running flag.

### 1.2 WndProc `FUN_0042B65C` message table

4-argument stdcall (`ret 0x10`); `hwnd = [ebp+0x14]`, `msg = [ebp+0x18]`,
`wParam = [ebp+0x1C]`, `lParam = [ebp+0x20]` (the prologue pushes ebx/esi/edi/ebp).
The dispatch is the range tree at `0x42B92A`:

| message | value | handler @ | effect |
|---|---|---|---|
| WM_CREATE | `0x0001` | `0x42B6C4` | return 0 |
| WM_DESTROY | `0x0002` | `0x42B6C9` | `FUN_0042987C()`, `FUN_00429744()`, `PostQuitMessage(0)` |
| WM_PAINT | `0x000F` | `0x42B847` | `BeginPaint`; if `_DAT_0081D5AC==0` → `FUN_0042CBD0(-1)` + `FUN_0042A470()`; `EndPaint` |
| WM_ACTIVATEAPP | `0x001C` | `0x42B6B7` | `_DAT_0082D2C8 = (byte)wParam` |
| WM_SETCURSOR | `0x0020` | `0x42B8DE` | `SetCursor(0)`; return 1 |
| WM_KEYDOWN | `0x0100` | `0x42B6F6` | **enqueue** `wParam` into the ring (flag 1) |
| WM_KEYUP | `0x0101` | `0x42B737` | if `wParam==0x10` enqueue code `0x10`; if `0x11` enqueue `0x11` (flag 0) |
| WM_MOUSEMOVE | `0x0200` | `0x42B812` | `_DAT_0081D5B8 = lParam.x − _DAT_0081D5C0`, `_DAT_0081D5BC = lParam.y − _DAT_0081D5C4` |
| WM_LBUTTONDOWN | `0x0201` | `0x42B7B6` | `_DAT_0081D5B0 |= 1`, `_DAT_0081D5B4 |= 1`, recompute delta |
| WM_LBUTTONUP | `0x0202` | `0x42B7F9` | `_DAT_0081D5B0 &= ~1`, `_DAT_0081D5B4 &= ~1` |
| WM_RBUTTONDOWN | `0x0204` | `0x42B882` | `_DAT_0081D5B0 |= 2`, `_DAT_0081D5B4 |= 2`, recompute delta |
| WM_RBUTTONUP | `0x0205` | `0x42B8C5` | `_DAT_0081D5B0 &= ~2`, `_DAT_0081D5B4 &= ~2` |
| WM_USER | `0x0400` | `0x42B8F3` | if `_DAT_0082D2C8` → `FUN_00436214()` **then** `FUN_0042A470()` |
| WM_USER+0x0D | `0x040D` | `0x42B6E1` | `FUN_00445D9A(0)`; if ≠0 `FUN_0042B098()` |
| WM_USER+0x1B9 | `0x03B9` | `0x42B6A7` | if `wParam==1` → `FUN_00429630()` |
| *(default)* | | `0x42B90B` | `DefWindowProcA(hwnd,msg,wParam,lParam)` |

Notes:
- `_DAT_0081D5B4` mirrors `_DAT_0081D5B0` as a "repeat/held" copy (used by the
  edge latch in Stage B).
- The mouse **delta** is `windows_cursor − game_cursor` (see §1.3); it is
  recomputed on every mouse message and on button-down (so a click resyncs after
  a sparse move stream).

### 1.3 Cursor integrator `FUN_00428C00` (called every frame by Stage B)

```
_81D5C0 += _81D5B8          ; cursor_x += dx
_81D5C4 += _81D5BC          ; cursor_y += dy
_81D5BC = 0 ; _81D5B8 = 0   ; consume delta
clamp _81D5C0 to [0, _81D740]
clamp _81D5C4 to [0, _81D75C]
```

So the *visible* command cursor `_81D5C0/_81D5C4` is the Windows pointer,
integrated from deltas and clamped to the viewport rectangle
`[0.._81D740] × [0.._81D75C]` (the detail-window bounds). `FUN_0042A470`
additionally copies `_81D5C0→_81D730` and `_81D5C4→_81D74C` each frame
(`FUN_0042A470` @`0x42AC33`).

### 1.4 Input event ring buffer

- `u32` events at `0x81C154[16]`; byte flags at `0x81CF70[16]`; write index
  `_DAT_0081D66C`, read index `_DAT_0081D668`; **length 16** (index `& 0xF`).
- Enqueue (WM_KEYDOWN, `0x42B6F6`):
  `flags[w] = 1; ev[w] = wParam; w = (w+1) & 0xF;`
- WM_KEYUP enqueues fixed codes `0x10`/`0x11` with `flags[w] = 0`.
- Both wrap and, when `_DAT_0081D5F4 != 0`, call `FUN_0042B098` (renders/limits).

---

## 2. Stage B — per-frame driver `FUN_00436214`

Runs once per 60 ms tick (from the `0x400` message). It:

1. Pops **one** event (`flag = _81CF70[r]; code = _81C154[r]; r = (r+1)&0xF`;
   empty → code 0).
2. Event handling:
   - `flag==0`: `code 0x10 → _DAT_0081D624 = 0`; `code 0x11 → _DAT_0081D5E4 = 0`.
   - `flag!=0`: `code 0x10 → _DAT_0081D624 = 1`; `code 0x11 → _DAT_0081D5E4 = 1`
     (and consume the code); `code 0x1B` (ESC) → screen switch on `_DAT_0081D5AC`.
3. Button-edge latch on `_DAT_0081D5B0` / `_DAT_0081D5B4` → latched edge state
   `_DAT_0081D69C`; it clears press bits of `_DAT_0081D5B0` once consumed.
4. In gameplay (`_DAT_0081D5AC == 0`, game not over) it runs the world/interface
   update (`FUN_00420F30`, `FUN_00421B50`, `FUN_00424520`, `FUN_0041D048`) and
   calls **`FUN_0042260C`** (gated by `_DAT_0081D61C` bit 0 and, just before,
   feeds any leftover key event to `FUN_00435B0C`).
   The keyboard half of this pipeline (ring, VK dispatch, modifiers, gates) is
   documented in **`KEY_INPUT_SYSTEM.md`**.

`_DAT_0081D624` and `_DAT_0081D5E4` are the two keyboard-derived modifier flags
(the only keys WndProc special-cases on key-up are VK `0x10`/`0x11`).

The ESC switch (`_DAT_0081D5AC`) maps the menu/screen states; `_DAT_0081D610`
(the interface mode used by Stage C) is changed by the commands below, not here.

---

## 3. Stage C — command compiler `FUN_004381C8`

Pure function `int FUN_004381C8(void)` (body `0x4381C8`–`0x43863D`). It reads the
cursor `_DAT_0081D5C0/_DAT_0081D5C4` and returns a **packed command**
`(index << 8) | code`, bailing with `-1` when nothing is under the cursor. Only
two callees:

- `FUN_00438798(x, y, rect_table)` — point-in-rectangle-list test → index or `-1`.
- `FUN_00438900(x, row, off_table, byte_table)` — **weighted in-row column
  lookup**: `off = off_table[row]` (`-1` = absent → 0), then walk `byte_table`
  (`value,weight` pairs, 2-byte stride) accumulating weights until the running
  sum exceeds `x`; returns the `value` byte. Used for the icon/inventory column
  hit-testing.

Gates by interface mode `_DAT_0081D610`:

| `_81D610` | meaning | command produced |
|---|---|---|
| `5` | **world map / travel** | rectangle test `0x462BEA…`; if hit and discovered-location flag `+0x81CF80[index] != 0` → `index*0x100 + 0x103` (travel), else `-1` |
| `2` | quest log | `FUN_00438798(...,0x45B04C)` → `index*0x100 + 0xCA` |
| `7` | inventory detail | weighted column lookups over `0x465E94`/`0x465E34`; `*0x100 + {0x78,0x79,0x32, 0x1E..0x27}` |
| `default` (0 world / 1 dialog / 3 character) | — | see below |

Default branch:
- `local = FUN_00438900(cursor, _81D584, _81D588)` (a UI region id / control).
- If `local == 0`: reject when `cursorY < _81D738 || _81D760 < cursorY` (outside
  the detail window).
- If `local < 8`: **return `local`** → world-region codes **0..7**.
- If `_DAT_0081D600 != 0`: return raw `local` when `local > 0x77`; else if the
  cursor is inside the inventory strip rect (`_81D734/_81D750/_81D760`) →
  `((cursorX-_81D734)/_81D724)*0x100 + 0x32` (inventory slot column, code `0x32`).
- If `local` in `0x1E..0x27` (party-member slot) → `(slot-0x1E)*0x100 + 0x1E`.
- `mode 1` (dialogue): scan the option rect list at `0x788DB4` (count
  `_81D6D0`, stride `0x20`) → `index*0x100 + 100` (hover highlight).
- `mode 3` (character screen): only accept `local` in `8..0x13` or `0x28..0x2D`.

---

## 4. Stage C — dispatcher `FUN_0042260C`

`FUN_0042260C` (body ≈`0x42260C`–`0x424E0C`, ~34 KB of pseudocode) is entered by
Stage B. Its top level is a **two-branch switch on the mouse buttons**:

```
if (_DAT_0081D5B0 & 1) {            /* LEFT button */
    code = FUN_004381C8() & 0xFF;  /* command compiler */
    <left-button command switch>
}
if ((_DAT_0081D5B0 & 2) == 0) {     /* NEITHER button (hover) */
    <hover: cell/object tooltip + hovered-actor name panel + UI tooltips>
}
/* RIGHT button only: */
<right-button world order/interaction switch>
```

`_DAT_0081D578` is the **actor under the cursor** (maintained in the
`0x4236xx`–`0x423Cxx` hit-test region of this function; several of those blocks
were folded away by the decompiler as "unreachable" — see the warnings at the
head of its listing). `_DAT_0081D56C` is the currently controlled hero pointer;
`_DAT_0081A814[0..9]` is the active/party unit list (≤10 pointers).

### 4.1 Left-button command switch (world + interface)

`code = FUN_004381C8() & 0xFF`; the high byte is an index (`index << 8`). All
handlers resolved from REA (the switch is world-only for code `0`; the rest are
interface commands that share it):

| code | index | handler | action |
|---|---|---|---|
| `0` | — | inline | **move the party** to the clicked cell (§4.4); world only |
| `1`, `0x65` (101) | — | `FUN_004216DC(0xFFFFFFFF)` | **cycle each member's active weapon slot** (`Entity+0x12` = 0/1/2, source slots `+0x66/+0x68/+0x6A`; drops battle stance unless slot 1) |
| `2` | — | `FUN_00421554` | **selected-unit battle/peace stance toggle** (`+0x00 ∓6`, `+0x03 ^= 0x04`; first selected decides direction) |
| `3` | — | inline | open **world map** / interface mode 4 (`FUN_00428E98`, sound `FUN_0042A13C(4)`, `FUN_0042CBD0(4)`; mode 5 → close) |
| `4` | — | `FUN_0042246C` | **party halt/stand** (`+0x02 = 0x60`, `+0x14 = 0xFF`, members ≥ 1) |
| `5` | — | `FUN_00421458` | **party battle-engage** (battle stance on, `+0x02 = 'D'/'A'` by `+0x12`, `+0x06` cleared, queue cleared; no target is assigned - `FUN_0041E2D4` re-acquires on the next tick and after every kill) |
| `6` | — | `FUN_004242BC` | toggle **quest-log** window (`FUN_0042CBD0(1)` → mode 2) |
| `7` | — | `FUN_00424330` | toggle **character/member** screen (`FUN_0042CBD0(2)` → mode 3) |
| `8`..`0xD` | — | `FUN_0041E450(n)` | **equip the selected item into main slot** `Entity+0x66+2n` (`n` = 4 / 0·2 / 5 / 3 / 1 / 0xC) |
| `0xE`..`0x12` | — | `FUN_0041E6BC(code)` | **equip into equipment slot** `member+0xB2+2*(code-0x0E)`; class-gated by table `0x659B0C` (classes `6/7/8`) |
| `0x13` | — | `FUN_0041E8A8` | **pick up / put down** the selected item: move the held item between the member's slots (primary `+0xCC`, main `+0x66..`, additional `+0xB2..`, inventory) |
| `0x1E` | 0..9 | inline | **party member selection** (index = the `slot-0x1E` high byte) |
| `0x28`..`0x2D` | — | `FUN_00410B08(entity,{0,3,5,2,6})` | stat/level spend |
| `0x32` | column | inline | inventory slot move/stack, or select item into `_81D560` |
| `0x64` (100) | option | inline | dialogue **option select** (`FUN_00434BB8`, next node, `FUN_00434C74`, `FUN_0042CBD0(0)`) |
| `0x78` / `0x79` | — | inline | scroll inventory left/right (`FUN_0042E520`, `FUN_00430D6C`) |
| `0xC8` / `0xC9` / `0xCA` | — | `FUN_004203F4` / `FUN_00431E2C` | close / trade / quest-log scroll |

Codes `6/7` toggle whole windows; `8..0x12` assign the selected inventory item
into an equipment slot; `1..5` are party orders (weapon slot / stance / halt /
engage). (`FUN_00430D6C(mode)` is the interface-mode opener; `FUN_0042CBD0(mode)`
renders the interface; `FUN_0042E520(entity,delta)` redraws the item/party panel.)

### 4.2 Hover branch (no button)

For mode 0:
```
FUN_00438AB4(&gx,&gy, cursorX+cam, cursorY+cam)     ; screen -> cell
if ((DAT_004EE6B0[gx*0x140 + gy*2 + 1] & 0x20) != 0
    && (obj = FUN_00410E10(gx,gy)), (*(int*)(obj+0x0E)>>16) >= 0)
        FUN_0042E9CC(&DAT_0045C31C);               ; "cell has object" tooltip
else if (_DAT_0081D578 != 0)
        FUN_00430958(_DAT_0081D578);               ; hovered-actor NAME panel
```
Then it emits UI tooltips: the seven-icon HUD strip uses the fixed strings
`0x45E11C/0x45D71C/0x45DF1C/0x45D91C/0x45D51C/0x45DB1C/0x45DD1C` for codes
`1..=7` (weapon/stance/map/halt/engage/quest/character), while
inventory/equipment/party-collage slots call `FUN_0042F5F8` over the
`0x659B0C` item-instance table. `FUN_00430958(actor)` composes the actor's
class+name+stats panel from `Entity+0x04` (archetype, `&0x7F`, `&0x40` =
non-human), `Entity+0xD4/+0xD5` (name/nickname), `Entity+0x05` (group) and the
`Entity+0x33/+0x5C/+0x50/+0x58/+0x5E/+0x54/+0x62` fields (level/health/poison/
base stats). The `Group+0x1D & 1` activity bit adds `, враг`; otherwise the
`FUN_0041182C` waypoint role (`0x46270C`) or the `+0x42/+0x46/+0x48/+0x4A/+0x4C`
skill thresholds are appended.

All four texts are drawn by `FUN_0042BD8C` with the small font (`0x58BC34`)
**left aligned** at `(_DAT_0081D738+10, _DAT_0081D760+5)`, wrapped to
`(right-left)-15`, 15 px line advance. At 800×600 that is (150, 560) with a
644 px wrap; at 640×480 it is (122, 449) with 512 px. The routine first re-blits
the HUD background clipped to that region, so there is no separate text box.

### 4.3 Right-button world order/interaction switch

Entry gate: `if (_DAT_0081D660 == 7) _DAT_0081D660 = 4; else if (_DAT_0081D660 != 4)
{ FUN_00428E98(); return; }`.

Then `code = FUN_004381C8() & 0xFF`:

- **`code == 0`** — world action (the important one; see §4.5).
- `code 0x1E` — party-member order branch (`FUN_00438104`, follow, `FUN_0042CBD0(-1)`).
- `code 0x32` — item use/`switch(item_class)`: classes `6/7/8/9/0xA/0xB` route to
  `FUN_00424E0C`+`FUN_0041E160`, `FUN_0041E6BC(0xE)`, `FUN_0041DAB0`,
  `FUN_00414DE4`, or the default `FUN_0041E450`.

### 4.4 Left-click code 0 — "move the party to this cell"

```
if (_81D610 != 0) return;                       ; only in the world
if (_81D660 == 7) _81D660 = 4;
FUN_00438AB4(&gx,&gy, (cursorX + _81D72C) - _81D738,
                     (cursorY + _81D748) - _81D754);      ; screen -> cell
if (_81D660 != 4) { _81D660 = 4; FUN_004138C8(w=..,h=..,_81D400); return; }
for (i = 0; i < 10 && DAT_0081A814[i]; i++) {
    u = DAT_0081A814[i];
    *(u16*)(u+0x0C) = gx;        ; destination cell x
    *(u16*)(u+0x0E) = gy;        ; destination cell y
    *(u8*)(u+0x02)  = 'H';       ; order byte (bit 0x40 route, bit 0x08)
    if (u != _81D56C) *(u+0x02) |= 2;   ; mark followers (0x4A)
    *(u8*)(u+0x14)  = 0xFF;      ; clear path-queue terminator
    *(u8*)(u+0x03) &= 0xBF;      ; clear +0x03 bit 0x40
}
```
`FUN_004138C8(x,y,type)` (called when `_81D660 != 4`) is a **per-cell marker
spawner**: it finds or creates one 64-byte record in a 200-entry pool at
`0x785BB4` (counter `_DAT_0081D6A8`, cap `0xC8`), keyed by cell x/y, appends the
type to a ≤22-entry list, sets the cell flag bit `0x20` in `DAT_004EE6B0`, and
computes a sprite-centered screen anchor via `FUN_00438A78`.

### 4.5 Right-click code 0 — the world click decision tree

```c
if (_81D610 != 0) return;
FUN_00438AB4(&gx,&gy, cursorX+cam, cursorY+cam);       /* cursor -> cell */

/* (1) nothing selected */
if (_81D578 == 0 && _81D57C == 0) {
    for each of ≤10 units DAT_0081A814:
        set destination = (gx,gy), order byte 'H', queue term 0xFF, +0x03 |= 0x40;
    return;                                   /* == move the party */
}

/* (2) clicked something different from the controlled hero */
if (_81D578 == 0 || (_81D578->+0x05 != _81D56C->+0x05)) {

    /* 2a. NPC with dialogue */
    if (_81D578 != 0 && *(u8*)(_81D578+0xD0) != 0xFF
        && _DAT_0081D624 == 0 && (_81D56C->+0x03 & 4) == 0)
    {
        _81D600 = 0;
        if (FUN_00435290(_81D578) != 0) FUN_0042CBD0(0);   /* open dialogue */
        return;
    }

    /* 2b. otherwise: issue orders to the party toward/targeting the actor */
    for each of ≤10 units DAT_0081A814:
        (if unit is "ready": flags +0x0F>>24==1 && +0x68!=0 && +0xCC!=0 && +0x03&4)
            set order byte 'D' (0x44) or 'A' (0x41) from weapon slot +0x12,
            point at the target, destination cell = clicked cell;
    return;
}

/* (3) clicked the controlled hero himself */
if (_81D5E4 != 0 && *(u8*)(_81D578+0xD0) != 0xFF) {
    _81D600 = 0; FUN_00435290(_81D578); FUN_0042CBD0(0);   /* open dialogue */
    return;
}
if (_81D624 == 0) FUN_00424D8C(_81D578);      /* (de)select */
else { if (FUN_00411708(_81D578)==0) FUN_00424D8C(_81D578); else FUN_00438104(_81D578); }
FUN_0042CBD0(-1);
… follow-up FUN_0042E520 for the hero / DAT_0081A814 …
```

The **dialogue guard** (matching `DIALOG_SYSTEM.md` §2) is therefore:

```
_DAT_0081D578 != 0
 && *(u8*)(_DAT_0081D578 + 0xD0) != 0xFF     /* dialogue_id present */
 &&  _DAT_0081D624 == 0                      /* modifier flag clear */
 && (_DAT_0081D56C[3] & 0x04) == 0           /* hero not in battle stance */
   ⇒ FUN_00435290(_DAT_0081D578); FUN_0042CBD0(0);
```

`FUN_00435290(entity)` selects `dialogue_id → first node` from `QUEST_R.RES` and
plays the archetype voice (`0x0C..0x13`); `FUN_0042CBD0(0)` renders interface
mode 1 (`_DAT_0081D610=1`). See `DIALOG_SYSTEM.md`.

### 4.6 What each click does (plain-language gates)

| You click… | Left button | Right button |
|---|---|---|
| empty **passable** cell | move every selected unit to the cell (§4.4) | same move, if no actor is targeted (§4.5.1) |
| **blocked / non-passable** cell | destination stored, but the route build fails ⇒ the unit **does not move** (§5) | same |
| **NPC** (human `archetype & 0x40 == 0`, `dialogue_id != 0xFF`) | move only | **open dialogue** (`FUN_00435290` + `FUN_0042CBD0(0)`) |
| **NPC without dialogue id** | move only | party order toward the actor (no dialogue) |
| **mob / enemy** (`archetype & 0x40`) | move only | party **attack/order** toward the actor (never a dialogue) |
| **prop/object sprite pixel** (`_DAT_0081D57C`) | move only | ranged prop order only with plain ammo (`+0x02 == 0`); target id `-(prop+1)`, clicked cell in `+0x0C/+0x0E` |
| nothing (hover) | — | — (hover panel/tooltip only) |

**Village role assignment is dialogue-only.** `konung.res` has no generic
`Ctrl + right-click` “leave villager in this village” command. Ctrl is the
modifier flag `_DAT_0081D5E4` (dialogue-on-own-hero, cursor variant, Ctrl+S
camera follow); it is not a stationing input. The original assigns elders and
specialists through dialogue opcodes `0x14C` and `0x159`–`0x15C`, which write the
settlement waypoint owner and copy the waypoint `+1` role dialogue id onto the
NPC. See `SETTLEMENT_SYSTEM.md`.

---

## 5. Movement, pathfinding and the passable/blocked decision

The click itself only **stores** the destination (`Entity+0x0C/+0x0E`, order byte
`+0x02`, `+0x14 = 0xFF`). The canonical "issue a move" routine is
`FUN_004142D4(entity, cellx, celly, action)` (`ev_987d36f8…`):

```c
*(u16*)(e+0x0C)=cellx; *(u16*)(e+0x0E)=celly; *(u8*)(e+2)=action; *(u8*)(e+0x14)=0xFF;
if (FUN_0043DA1F(e) == 0) return 0;             /* route failed -> NO move */
*(u8*)(e+2) &= 0xBF;                            /* clear the +0x02 route bit 0x40 */
FUN_0041435C(e, (*(u8*)(e+3) & 4) ? 1 : 7);     /* walk anim (battle/idle) */
return 1;
```

**Route building happens on the next per-frame tick, not in the click handler.**
`FUN_00436214` calls the group/actor dispatcher **`FUN_00421B50`**, which routes
each group by class (`Group+0x1E`): `&0x40` → `FUN_004123BC` (object/enemy),
`&0x80` → `FUN_00415FC8` (citizen, delegating movement to `FUN_004168A4`),
`&0xC0==0` → inline per-actor switch. The **walk/idle cases** build the route via
**`FUN_0041ECC8` → `FUN_0043DA1F` → `FUN_0041435C`** (the player/party path),
while the object/enemy path uses `FUN_004142D4`. `FUN_0043DA1F` is the
collision-aware pathfinder (searches the Konung.N collision grid `+0x06400`,
320×160, `index = x*160 + y` over the parity-selected 8-neighbour set; see
`KONUNG_SOURCE_OF_TRUTH.md` §1.1/§3 and AGENTS.md); it is called with the entity
only and reads the stored destination from the struct. So
**the collision decision is made when the unit processes its stored order**, not
in the click handler itself:

- **passable & reachable** → a direction queue is written to `Entity+0x13`, the
  walk animation starts, and the unit steps cell-by-cell;
- **blocked or unreachable** → `FUN_0043DA1F` returns 0 → `FUN_004142D4` returns
  0 → the target stays stored but **no walk starts** (the unit never enters a
  blocked cell and never moves to an unreachable goal).

`FUN_00438AB4` (the cursor→cell inverse of the verified projection
`sx = y*58 + (2-(x&1))*29, sy = x*16 + 16`) clamps its output to
grid `[0..0x13F] × [0..0x9F]` (320×160), so an off-map click resolves to the
nearest border cell.

---

## 6. Globals quick reference

| global | role |
|---|---|
| `_DAT_0081D5B0` | mouse buttons: bit0 left, bit1 right |
| `_DAT_0081D5B4` | held/repeat mirror of the buttons |
| `_DAT_0081D5B8` / `_DAT_0081D5BC` | accumulated mouse dx / dy |
| `_DAT_0081D5C0` / `_DAT_0081D5C4` | integrated, clamped cursor x / y |
| `_DAT_0081D5C8` | frame budget (`0x3C` = 60 ms) |
| `_DAT_0081D5AC` | top-level screen/state id (0 = in-game; 1..9 menus/screens) |
| `_DAT_0081D610` | interface mode (0 world / 1 dialog / 2 quest / 3 character / **5 world map-travel** / 7 detail) |
| `_DAT_0081D660` | base cursor/action id `{0,2,4,7,8,9,10}` (4 = neutral); also the `==4` assign-mode gate |
| `_DAT_0081D668` / `_DAT_0081D66C` | ring read / write index |
| `_DAT_0081D624` / `_DAT_0081D5E4` | keyboard modifier flags (VK `0x10`/`0x11` edges) |
| `_DAT_0081D578` | actor under the cursor |
| `_DAT_0081D55C` | current-location/context pointer (party count byte at `+0x1C`) |
| `_DAT_0081D56C` | leader/party Entity pointer (`+0x03 & 4` = battle stance gate) |
| `_DAT_0081A814[0..9]` | party pointer list (`[0] = _DAT_0081D56C`) |
| `_DAT_0081D600` / `_DAT_0081D708` / `_DAT_0081D670` | inventory/trade selection state |
| `_DAT_0081D638`/`…` | detail-window rect (`_81D738/40/44/54/5C/60`, `_81D72C/48`, `_81D724`) |
| `_DAT_004EE6B0` | 320×160 u16 per-cell flag map (x-stride `0x140`; bit `0x20` = has object) |
| `0x785BB4` | 200 × 0x40 cell-marker/effect record pool (`_DAT_0081D6A8` = count) |
| `0x67C08C` | actor name/record table, stride `0x10`, indexed by `Entity+0x05` |

---

## 7. Original mouse cursor

The game hides the Win32 cursor and draws its own: `LoadCursorA(NULL, 0x7F00)`
(= `IDC_ARROW`) is only the WNDCLASS cursor; `ShowCursor(0)` (`0x42B574`) and
`SetCursor(0); return 1` in WM_SETCURSOR (`0x42B8E0`) hide it.

- **Bitmaps:** 11 × 32×32, the first blocks of `GRAPH.RES` at offset `0x20000`
  (`FUN_004392B4` copies them into the RAM table `0x81A83C`; `res/graph.rs`
  already steps over the same 11 blocks). Format `[u32 type=0x05000000]
  [u32 size]`, then a **16-bit color** sprite `[u16 w][u16 h][h×u16 row-size]
  [rows]`: `0x00` ends a row, `b & 0x80` skips `(b & 0x7F)` pixels, else `b`
  literal 2-byte pixels (`FUN_0043D2FE` copies raw 16-bit words, no palette).
- **Selection `FUN_00428740`** (per frame, gameplay only). The whole hover
  block runs only when `_DAT_0081D5AC == 0`, `_DAT_0081D5F4 == 0`,
  `_DAT_0081D610 == 0`, `_DAT_0081D660 == 4` and the pointer is inside the world
  viewport (`_DAT_0081D738 <= x <= _81D744`, `_81D754 <= y <= _81D760`); when
  `_81D600 != 0` the inventory strip row (`y >= _81D750 - 1`) keeps the base id.
  Otherwise, in priority:
  - `6` — an interactive prop (`_81D57C`, prop `+0x22 & 1`) is under the pointer,
    `_81D580` (settlement) is set and a **selected** living unit is ranged-ready:
    battle stance (`Entity+0x03 & 4`), active weapon slot `+0x12 == 1`, a bow in
    `Equipment[1]`, ammo in `+0xCC`, and plain ammo (instance `+0x02 == 0`).
  - actor under the pointer (`_81D578`): same group as the leader (`+0x05`) →
    `5` when Ctrl (`_81D5E4`) else `4`; otherwise `1` when any selected living
    unit is in battle stance; otherwise `3` when the actor is non-talkable
    (`+0xD0 == 0xFF`), Shift (`_81D624`) is held or the leader is in battle
    stance; else `5`.
  - interactive prop without a ready archer → base `4` (blocks the ground).
  - free ground cell flagged `0x2000` → `7` (ground-object hand).
  - occupied cell (static low 12 bits or a live entity) → `3`.
  - other free ground → base `4`.
- **Ground objects `0x2000`.** The map loader `FUN_0043ADF0` (`0x43BC…`) ORs
  `0x20` into the cell's high byte for every active ground marker (herbs, money,
  loot); `FUN_0041093C` XORs it off when the pile is emptied and `FUN_00418480`
  sets it for corpse loot. This is why the hand appears over collectables even
  when the pointer is not on an opaque sprite pixel.
- **Base id `_DAT_0081D660`** is set by `FUN_00424E0C` from the active
  item/action class (`0..=0xC`): `0,2→10` (sword), `1,12→2` (bow), `3..5→0`
  (chest/object), `9→8` (green/location), else `9` (creature), and is reset to
  `4` by `FUN_00428E98` when the held item is placed/cancelled. While it is not
  `4` the hover override is disabled everywhere. All 11 frames are reachable —
  `1,3,5,6,7` from the hover override plus `0,2,4,8,9,10` from the base; the
  class-11/subtype-`0x18` use effect additionally forces `7`
  (`FUN_00414DE4` `0x415484`).
- **Blit `FUN_0042A470`** saves a 32×32 patch (`DAT_00623318`), selects with
  `FUN_00428740`, and blits at `_DAT_0081D5C0/_D5C4` via `FUN_0043D2FE`.

**Remake:** `res::cursor` decodes all 11 frames (`res::cursor_image`, accessor
`Res::cursor_sprite`) from `GRAPH.RES`; `app::cursor` ports the whole selection:
`select_cursor` is the pure precedence table and `cursor_index` assembles its
`CursorHover`/`CursorState` from the world/party (viewport via
`hud::layout().viewport`, props via `Prop::interactive` /
`interactive_prop_at_local`, ground flags via `Location::collision_cell` plus
`active_world_objects`, live actors via the population/party cells, Shift/Ctrl via
`shift_down`/`ctrl_down`). `draw_detail` blits the frame at `cursor_virtual`;
`www/app.js` hides the OS cursor in game mode and syncs the modifiers
(`set_shift`/`set_ctrl`). The remake always treats the inventory strip row as
interface (`_81D600` assumed set, since its HUD strip always shows a member).
Diagnostic export: `tools/cursor_png.mjs` ->
`tools/cursor_dump/cursor_NN.png` (ABI `debug_cursor_count` /
`debug_decode_cursor_image`). The class-11/subtype-`0x18` forced `7` base id is
**not** reachable yet because the remake has no class-11 use effects.

---

## 8. Remake cross-reference (`web/src/`)

| Original | Remake (`app/input.rs`, `dialog.rs`) | Status |
|---|---|---|
| Dialog opens on **right**-click (`_81D5B0 & 2`, code 0) | `pointer_down(button==3)` → `world_interact` opens the dialogue | **matches** |
| Left-click = move whole party (10 units) | left-click → `world_move` (move only, gated on `party_selected[0]`) | matches (leader gate) |
| Right-click code 0 also moves when no actor is targeted | right-click `world_interact` falls through to `world_move` | matches |
| Right-click = interact (attack/order for non-dialogue actors) | non-dialogue actor -> melee order to selected units in battle stance (`0x44`/`0x41` equivalent); no move when no unit is in stance; no longer toggles the graph | matches |
| Graph view returns with `H`/`Backspace` | `key_down` `KEY_H`/`KEY_BACKSPACE` (right-click no longer used) | remake change |
| Middle button unused for gameplay | `button==2` = camera drag | remake-only |
| Hover shows actor **name/stats panel** (`FUN_00430958`) | `app/hover.rs` builds the original name/class/level/health/party/profession/monster panel; `hud::draw_hover_text` draws it left aligned | **matches** |
| Hover shows **ground/item/HUD-button** tooltips | ground marker `0x45C31C`, full `FUN_0042F5F8` item detail, fixed strip strings `0x45D91C..` | **matches** |
| Move = store cell; route via `FUN_0043DA1F` on the unit's tick | `set_target_from_mouse` → `actors::find_path` | conceptually matches |
| World-click stores the destination; `FUN_004142D4` is the actor-logic destinaton+route+wake routine (AGENTS.md previously said the click issues it — corrected) | — | **corrected** |

---

## 9. Resolved questions

All five former open questions were resolved with REA:

1. **`_DAT_0081D660`** is the **base cursor/action id** (set by
   `FUN_00424E0C` from the item/action class; `4` = neutral; the class-11
   subtype-`0x18` effect forces `7`), not a pure order state; the hover override
   only applies while it is `4`. **`_DAT_0081D5AC`** is the **top-level
   screen/state id `0..9`** (`-1` init `0x42B1B4`, `9` `0x42B2B0`, `0..8` in
   `FUN_00436214`); `0` = in-game.
2. The click bytes are **order-byte `Entity+0x02`** (`0x48 'H'` = move, bit `0x40`
   route + `0x08`; `0x44 'D'` / `0x41 'A'` = engage, chosen from the active weapon
   slot `+0x12`; `0x60` = halt) with **`Entity+0x03`** flags (bit `0x04` battle
   stance; bit `0x40` set on right-click, cleared on left). The *action row* is
   `Entity+0x00` (idle 0/6, walk 1/7, attack 2/8, death 3/9).
3. The full left-button code tree is enumerated in §4.1 (codes `1..0x12` named:
   weapon-slot cycle, stance toggle, halt, engage, window toggles, equipment
   slots); `FUN_0041E450` and `FUN_0041E6BC` are item/equipment-slot assignment.
4. **`FUN_00421B50`** (per-frame, from `FUN_00436214`) builds the route on the next
   tick: group class `&0x40` → `FUN_004123BC`, `&0x80` → `FUN_00415FC8` →
   `FUN_004168A4`, else inline switch → **`FUN_0041ECC8` → `FUN_0043DA1F` →
   `FUN_0041435C`**. The party is `0x81A814` (`[0] = _DAT_0081D56C`; members are
   `0x100`-stride records at `0x78A250`), so it is processed by the same tick.
5. **`FUN_0042A470`** is the per-frame compositor/presenter (DirectDraw blit of
   the window sprite lists) that **also** copies `_81D5C0/_D5C4 → _81D730/_81D74C`
   and draws the cursor — "present/blit" and "cursor" are the same routine.

### 9.1 `Entity+0x02` / `Entity+0x03` bit meanings (resolved)

`Entity+0x02` — **order/state byte**:

| bit | meaning | evidence |
|---|---|---|
| `0x01` | **active move/engage order** (low bit of `0x41 'A'`) | `FUN_0041F048` gates on it (`0x41F10F`): clear → actor skipped; set (walk row 1/7) → re-issues the route. `FUN_0041EB48` treats `0x09` as "ordered/busy" |
| `0x02` | **target/follow lock** | `FUN_0041EB48` `OR …,0x2` on acquiring an adjacent enemy; matches the left-click follower marker (`0x4A`) |
| `0x04` | **target/approach mode** | `FUN_0041ECC8` `TEST …,0x4` (clear → walk/idle; set → target branch); toggled by the weapon-slot cycle |
| `0x08` | **move order / arrived** (low bit of `0x48 'H'`) | `FUN_004123BC` clears it on arrival (`&0xF7`) |
| `0x20` | **autonomous AI enabled** | `FUN_0041EFD8` (`TEST …,0x20`): clear → no-op; halt sets `0x60 = 0x40\|0x20` |
| `0x40` | **route pending/needed** | OR'd while a route is required, cleared on a successful route/arrival (`&0xBF`); present in `0x48/0x44/0x41/0x60` |

`Entity+0x03` — **flags byte**:

| bit | meaning | evidence |
|---|---|---|
| `0x02` | **additional-inventory / bag toggle** | command `0x2D` in `FUN_0042260C` (`0x422F5A`) toggles it + `FUN_0041079C` |
| `0x04` | **battle stance** | pervasive `TEST …,0x4`; selects walk anim 1 vs 7 |
| `0x40` | **arrival / occupy flag** | click clears (left) / sets (right); `FUN_00421B50` toggles it on arrival (`0x422068`) |
| `0x80` | **object/monster graphics-rebuild pending** (`+0x04 & 0x40` family) | set at map load when the type is first registered (`FUN_0043ADF0`, `0x43B52E`; `0x81B954` `-1→-2`, active list `0x81C194`); while set, `FUN_0041435C`/`FUN_00414078` substitute a placeholder frame (`+0xF4 = 1`); the renderer `FUN_0042532C` clears it (`0x425909`) and rebuilds `+0xDC/+0xE0/+0xF4` via `FUN_00414078` |

Code `0x13` (`FUN_0041E8A8`) is also resolved: an inventory/equipment **pick-up /
put-down** handler (moves the held item between the member's slots).
