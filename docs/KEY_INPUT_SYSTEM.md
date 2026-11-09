# Konung keyboard / key-press input — reverse-engineered notes

Target: `konung.res` (shipped PE32/i386, image base `0x400000`,
`sha256 d698c4f231489a372cc25212ab2819720a36d7ce5307a045db684135cf679810`).
All addresses are **virtual addresses in `konung.res`** unless stated otherwise.
This document is **authoritative to the original binary**; the remake is not
consulted for semantics here (see §10 for the only cross-reference).

Companion document: `MOUSE_INPUT_SYSTEM.md` describes the mouse half of the same
pipeline. This file describes only the **keyboard** half: every Win32 key
message, the ring buffer, the per-frame consumer, the exact VK dispatch, and
every gate that can swallow a keystroke.

> Evidence anchors (REA Ghidra provider 12.1.4; `analyze_function`/`xrefs`
> dossiers and Cursor-paged pseudocode retained in this session):
> `FUN_00435B0C` = `ev_f4c388a0…`; `FUN_00436214` = `ev_82f5d720…`;
> WndProc KEYDOWN @`0x42B6F6` = `ev_cba6a740…` + `ev_2ab8cb93…`;
> KEYUP @`0x42B737` = `ev_6316174f…`; `FUN_0042CBD0` = `ev_10f4bd95…`;
> `FUN_0042E520` = `ev_ff6cac26…`; `FUN_00430D6C` = `ev_5f6de4ed…`;
> `FUN_004392B4` = `ev_ad7e413b…`; `FUN_004182D4` = `ev_21ccd9d4…`.

> **Note on the WndProc.** Ghidra defines **no function** at the WndProc
> (`FUN_0042B65C` region); `resolve_containing_procedure(0x42B6F6)` returns
> `not_in_procedure`. The key-message handlers below were read as raw
> instructions (`inspect_native_instruction` on exact byte boundaries) and via
> the range-tree dispatch table already documented in `MOUSE_INPUT_SYSTEM.md`
> §1.2.

---

## 0. Pipeline overview

```
   Windows USER32 message queue   (class "Konung", registered in FUN_0042B1A8)
                 │
                 ▼
(A)  WndProc  ~0x42B65C  (range tree @ 0x42B92A; NOT a Ghidra function)
       WM_KEYDOWN 0x0100 @ 0x42B6F6  ->  enqueue(wParam, flag=1)
       WM_KEYUP   0x0101 @ 0x42B737  ->  enqueue(0x10|0x11, flag=0)   [only Shift/Ctrl]
                 │
                 ▼
     input event ring:  ev[16] u32 @ 0x81C154, flags[16] u8 @ 0x81CF70
                        write idx _DAT_0081D66C, read idx _DAT_0081D668 (& 0xF)
                 │
     frame tick  WM_USER 0x400  (60 ms budget, _DAT_0081D5C8 = 0x3C)
                 ▼
(B)  FUN_00436214   pops exactly ONE event per tick;
        - updates Shift/Ctrl flags _81D624 / _81D5E4
        - consumes ESC (0x1B) and runs the _81D5AC screen switch
        - in gameplay (screen 0, not game-over) and gated by _81D61C bit 0:
              if (leftoverVK != 0) FUN_00435B0C(leftoverVK);
              FUN_0042260C();                       (mouse/UI)
                 ▼
(C)  FUN_00435B0C(vk)   the keyboard command dispatcher (sole caller 0x436214)
```

**Timing / capacity consequence.** Because the producer enqueues on every
`WM_KEYDOWN` but the consumer pops **one event per 60 ms frame** (~16.7 Hz),
at most one keystroke is acted on per frame; the 16-slot ring absorbs a short
burst. There is **no queue-full check** — the producer only ever does
`w = (w+1) & 0xF`, and "empty" is encoded as `read == write`, so a full ring and
an empty ring are indistinguishable.

---

## 1. Stage A — Win32 key messages

WndProc is a 4-argument stdcall; `wParam = [ebp+0x1C]`, `msg = [ebp+0x18]`.
The range tree at `0x42B92A` routes the key messages (see
`MOUSE_INPUT_SYSTEM.md` §1.2 for the full table):

| message | value | handler @ | effect (ORIGINAL-CODE VERIFIED) |
|---|---|---|---|
| WM_KEYDOWN | `0x0100` | `0x42B6F6` | `EAX = [0x81D66C]`; `flags[EAX] @0x81CF70 = 1`; `EAX = [0x81D66C]`; `ev[EAX*4] @0x81C154 = wParam`; `w = (w+1) & 0xF`; if `_81D5F4 != 0` → `FUN_0042B098()` |
| WM_KEYUP | `0x0101` | `0x42B737` | `wParam = [ebp+0x1C]`; **only** `0x10` (VK_SHIFT) or `0x11` (VK_CONTROL) are enqueued, always with `flags = 0` |

Decoded instructions:

```
0x42B6F6  MOV EAX,[0x0081D66C]                    ; ring write index
0x42B6FB  MOV byte ptr [EAX+0x0081CF70],0x1        ; flags[w] = 1
0x42B702  MOV EAX,[0x0081D66C]                    ; reload index
          ... store wParam into ev[w], w=(w+1)&0xF, call FUN_0042B098 ...
0x42B737  MOV EAX,[EBP+0x1C]                      ; WM_KEYUP: read wParam
```

Consequences:

- **Any** key-down is recorded; the dispatcher, not WndProc, decides meaning.
- Key-up is **ignored** except for `VK_SHIFT`/`VK_CONTROL`, which are the only
  two "level" modifiers the game tracks (no Alt, no per-key release, no
  auto-repeat filtering — auto-repeat arrives as repeated WM_KEYDOWNs).
- No DirectInput; ordinary window messages only.

### 1.1 Ring buffer

| symbol | address | type | role |
|---|---|---|---|
| `ev[16]` | `0x81C154` | u32 × 16 | raw `wParam` (VK) per slot |
| `flags[16]` | `0x81CF70` | u8 × 16 | 1 = key-down, 0 = key-up |
| `_DAT_0081D66C` | `0x81D66C` | u32 | write index |
| `_DAT_0081D668` | `0x81D668` | u32 | read index |

Effective size **16** (`& 0xF`). Producer and consumer both wrap identically.

---

## 2. Stage B — per-frame consumer `FUN_00436214`

Runs once per 60 ms tick (`WM_USER 0x400`, `ev_82f5d720…`). The key-related
portion is at the top of the function:

```c
r = _DAT_0081D668;
if (r == _DAT_0081D66C) {          /* empty */
    code = 0;
} else {
    flag = *(u8*)(r + 0x81CF70);
    _DAT_0081D668 = (r + 1) & 0xF;
    code = *(u32*)(r*4 + 0x81C154);
    if (flag == 0) {                       /* WM_KEYUP */
        if (code == 0x10)      _DAT_0081D624 = 0;              /* Shift up  */
        else if (code == 0x11) _DAT_0081D5E4 = 0;              /* Ctrl  up  */
    } else {                               /* WM_KEYDOWN */
        if (code == 0x10) { _DAT_0081D624 = 1; code = 0; }     /* Shift down, consumed */
        else if (code == 0x11) { _DAT_0081D5E4 = 1; code = 0; }/* Ctrl  down, consumed */
        else if (code == 0x1B) {                               /* ESC, consumed */
            code = 0;
            switch (_DAT_0081D5AC) { ... }                     /* see §2.1 */
        }
    }
}
```

So `code` (the "leftover" VK) is **0** for Shift/Ctrl/ESC and is the raw VK for
every other key. `_DAT_0081D624` = Shift level, `_DAT_0081D5E4` = Ctrl level.

Then the function latches the mouse button edges, integrates the cursor delta
(`FUN_00428C00`), and runs `switch(_DAT_0081D5AC)` on the screen state. The
**gameplay** case (screen `0`, game not over) is where keys are dispatched:

```c
case 0:
  ... world/interface update (FUN_00420F30, FUN_00421B50, FUN_00424520, FUN_0041D048) ...
  if (!game_over) {
      if ((DAT_0081D61C & 1) == 0) {
          if (leftoverVK != 0) FUN_00435B0C(leftoverVK);      /* call site 0x4367E5 */
          FUN_0042260C();                                     /* mouse/UI */
      } else {
          DAT_0081D61C ^= 1;
          if (leftoverVK != 0) FUN_00435B0C(leftoverVK);      /* call site 0x43680C */
          FUN_0042260C();
          if ((DAT_0081D61C & 1) == 0) FUN_00428DDC();
      }
  } else {
      _DAT_0081D6B8 = 7; _DAT_0081D5F8 = 0;
      FUN_0042ADE4("GAMEOVER.FLM");
  }
  break;
```

`FUN_00435B0C` has **exactly one caller** (`0x436214`) and two call sites
(`0x4367E5`, `0x43680C`) — both inside this gameplay branch.

### 2.1 ESC screen switch (`switch (_DAT_0081D5AC)`)

ESC is handled in Stage B, **before** the screen switch, so it works on any
screen (not only gameplay) and is consumed (never reaches `FUN_00435B0C`).
`_81D5F8` = "game was running", `_81D6B8` = which menu page, `_81D610` =
interface mode.

| `_81D5AC` | action |
|---|---|
| `0` (in-game) | `FUN_00428E98()`; if `_81D610 == 1` (dialogue) nothing; else if `_81D610 == 0` → `_81D5AC = 7; _81D5F8 = 1; _81D6B8 = 0` (**open main menu**, remember game running); else → `_81D610 = 0; _81D680 = 0; FUN_00428F94()` (**close the interface window**) |
| `1` | if `_81D5F8 != 0` → `_81D5AC = 0; _81D6B0 = 0; FUN_0042C384(); FUN_0042CBD0(-1); FUN_00428F94()` (**resume the running game**) |
| `2` | `_81D5AC = 7; _81D5F8 = 0; _81D6B8 = 2` (menu page 2) |
| `4`, `5` | `_81D5AC = 7; _81D6B8 = 5` (menu page 5) |
| `6` | `_81D5AC = 7; _81D6B8 = 6`; copies state `_81D5AC..` → `_81D5D4/_5EC/_604/_60C/_620/_62C` from `0x831C54..` (menu page 6) |
| any other (`3`, `7`, `8`, `9`, …) | **default: no-op** — ESC does not act on these screens |

The ESC switch is separate from the *outer* `switch(_81D5AC)` screen dispatch in
the same function (which handles the per-screen game loop, including cases `8`
map-load and `9` poster); those outer cases are reached by the normal frame
flow, not by ESC.

`_DAT_0081D6B8` is the menu page selector; `_DAT_0081D5AC` is the top-level
screen id. (Cross-referenced with `MOUSE_INPUT_SYSTEM.md` §0/§6.)

---

## 3. Stage C — the key dispatcher `FUN_00435B0C(vk)`

Body `0x435B0C`–`0x436211` (non-contiguous; ~1778 bytes). Sole caller
`FUN_00436214`. Input `vk` is the raw Win32 virtual-key code that survived the
`code != 0` filter. Two global gates are read throughout:

- `_DAT_0081D624` — **Shift** level (set by Shift key-down in Stage B).
- `_DAT_0081D5E4` — **Ctrl** level (set by Ctrl key-down in Stage B).
- `_DAT_0081D638` — **debug/cheat enable** flag (see §5).
- Selection list `_DAT_0081A814[0..9]` (party pointers; `[0] = _DAT_0081D56C`,
  the controlled hero / leader).

The dispatch is a nested range tree on `vk`. The complete, literal behaviour:

```c
void FUN_00435B0C(uint vk) {
  if (vk < 0x49) {
    if (vk < 0x33) {
      if (vk < 0x30) {
        if (vk < 9) return;                       /* 0x00..0x08 ignored */
        if (vk > 9) {
          if (vk != 0x20) return;                 /* 0x0A..0x1F ignored (except SPACE) */
          FUN_00421554(); return;                 /* SPACE — battle/peace stance */
        }
        /* vk == 0x09 TAB */  ...see §4 TAB...    return;
      }
      /* 0x30..0x32  '0' '1' '2' */
      if (vk > 0x30) {                            /* '1','2' */
        if (vk > 0x31) { if (_81D638) _81D764 = 2; FUN_004216DC(0); return; }  /* '2' */
        /* '1' */        if (_81D638) _81D764 = 1; FUN_004216DC(2); return;
      }
      /* '0' falls to the debug tail below */
    } else {                                       /* vk >= 0x33 */
      if (vk < 0x34) { if (_81D638) _81D764 = 3; FUN_004216DC(1); return; }   /* '3' */
      if (vk > 0x41) {                             /* 'B'..'H' (0x42..0x48) */
        if (vk > 0x45 && vk < 0x47) {              /* 'F' */
          if (_81D638) _81D764 = 0xf; FUN_0042246C(); return;   /* F — party halt */
        }
        goto LAB_00435C8D;                         /* B,C,D,E,G,H */
      }
      if (vk > 0x39) {                             /* 0x3A..0x41 */
        if (vk != 0x41) return;                    /* 0x3A..0x40 ignored */
        if (_81D624 != 0) { ...select-all-visible...; FUN_0042CBD0(-1); return; } /* Shift+A */
        FUN_00421458(); return;                    /* A — party battle-engage */
      }
      /* 0x34..0x39 '4'..'9' */
      if (_81D638 != 0) _81D764 = vk - 0x30;        /* 4..9 (debug only) */
    }
  } else {                                         /* vk >= 0x49 */
    if (vk < 0x4a) { FUN_00424330(); return; }      /* 'I' — character window (mode 3) */
    if (vk > 0x53) {
      if (vk < 0x55) {                              /* 'T' (0x54) — trade toggle */
        _81D614 = (_81D614 == 0);
        if (_81D614 == 0) return;
        if (_81D610 == 7) { FUN_00430D6C(0); return; }
        if (_81D610 == 0) return;
        FUN_0042CBD0(_81D610 - 1);
        if (_81D610 == 3) FUN_0042E520(_81D560, 0);
        return;
      }
      if (vk > 0x6a) {                              /* numpad region */
        if (vk < 0x6c) { if (_81D634 < 2) _81D634++; return; }  /* VK_MULTIPLY 0x6A, VK_ADD 0x6B */
        if (vk < 0x6d) return;                     /* VK_SEPARATOR 0x6C */
        if (vk < 0x6e) { if (_81D634) _81D634--; return; }      /* VK_SUBTRACT 0x6D */
        if (vk != 0xc0) return;
        /* VK_OEM_3 '~' 0xC0 — detail of selected unit: */
        if (_81D610 != 0) return;
        _81D708 = 0; _81D670 = 0;
        if (leader == 0) { _81D560 = 0; _81D600 = 0; _81D630 = 0; return; }
        if (leaderIndex == _81D600) { _81D560 = 0; _81D600 = 0; _81D630 = 0; return; }
        FUN_0042E520(leader, 0); return;
      }
      if (vk < 0x58) return;                        /* 0x55..0x57 ignored */
      if (vk < 0x59) { if (_81D638) _81D764 = 0x12; return; }   /* 'X' (debug) */
      if (vk != 0x5a) return;                       /* 'Y' ignored */
      if (_81D638) _81D764 = 10; return;            /* 'Z' (debug) */
    }
    if (vk > 0x4c) {
      if (vk < 0x4e) {                              /* 'M' (0x4d) — world map toggle */
        if (_81D610 != 0 && _81D610 != 5) return;
        FUN_0042A13C(4,0,0,0); FUN_00428E98(); FUN_00428DB0(2,0xe);
        if (_81D610 == 5) { _81D610 = 0; FUN_00428F94(); return; }
        FUN_0042CBD0(4); return;                    /* arg 4 -> mode 5 */
      }
      if (vk < 0x51) return;                        /* 'N','P' ignored */
      if (vk > 0x51) {                              /* 'R','S' */
        if (vk != 0x53) return;                     /* 'R' ignored */
        if (_81D5E4 == 0) return;                   /* Ctrl required */
        _81D564 = (_81D564 != 0) ? 0 : _81D56C;     /* Ctrl+S toggle */
        return;
      }
      FUN_004242BC(); return;                        /* 'Q' — quest-log (mode 2) */
    }
    if (vk > 0x4a) {                                 /* 0x4B,0x4C */
      if (vk != 0x4c) return;                        /* 'K' ignored */
      if (_81D624 == 0) return;                      /* Shift required */
      if (_81D5E4 == 0) return;                      /* Ctrl required */
      _81D638 = (_81D638 == 0);                      /* Shift+Ctrl+L: toggle debug */
      return;
    }
LAB_00435C8D:                                        /* reached by fall-through for 'J' (0x4A) */
    if (_81D638 != 0) _81D764 = vk - 0x37;           /* B..H (not F) -> 0xB..0x11 ; J -> 0x13 */
  }
}
```

> Note on the **two** debug tails: `_81D764 = vk - 0x30` is the tail of the
> `vk < 0x49` branch (reached by `'0'`..`'9'`, giving 0..9);
> `_81D764 = vk - 0x37` is `LAB_00435C8D` (reached by the `goto` for
> `'B'`..`'H'` except `'F'`, and by fall-through for `'J'`, giving
> `0xB..0x11` and `0x13`). Only the written values are observed; the field's
> meaning is **UNKNOWN** (§5).

### 3.1 Literal key table

Addresses are the handler functions; "gate" lists what must hold for the action.

| VK (hex) | key | handler | action (observed) | gate |
|---|---|---|---|---|
| `0x09` | TAB | inline | cycle the active party selection: if 0–1 selected, select leader / next; if ≥2, clear (`FUN_0043EAD0(0x81A814,0,0x28)`); `FUN_0042CBD0(-1)`; if mode 3 or `_81D600` set → `FUN_0042E520` (open detail) | gameplay |
| `0x1B` | ESC | Stage B | screen/menu switch (§2.1) | any screen |
| `0x20` | SPACE | `FUN_00421554` | toggle battle/peace stance of selected units (`+0x00 ∓6`, `+0x03 ^= 4`, re-idle `FUN_00414078`/`FUN_0041435C`) | gameplay |
| `0x31` | `1` | `FUN_004216DC(2)` | select weapon slot `+0x6A` (`Entity+0x12 = 2`) | gameplay |
| `0x32` | `2` | `FUN_004216DC(0)` | select weapon slot `+0x66` (`Entity+0x12 = 0`) | gameplay |
| `0x33` | `3` | `FUN_004216DC(1)` | select weapon slot `+0x68` (`Entity+0x12 = 1`) | gameplay |
| `0x30` | `0` | inline | `_81D764 = 0` | `_81D638` (debug) |
| `0x34`–`0x39` | `4`–`9` | inline | `_81D764 = 4..9` | `_81D638` (debug) |
| `0x41` | `A` | `FUN_00421458` (no Shift) / inline (Shift) | Shift → select all visible units; else party battle-engage (`+0x02 = 'D'`/`'A'`, `+0x14 = 0xFF`) | gameplay |
| `0x46` | `F` | `FUN_0042246C` | party halt/stand (members ≥1: `+0x02 = 0x60`, `+0x14 = 0xFF`) | gameplay |
| `0x42`–`0x45`, `0x47`, `0x48` | `B`,`C`,`D`,`E`,`G`,`H` | `LAB_00435C8D` | `_81D764 = vk-0x37` (0xB..0x11) | `_81D638` (debug) |
| `0x49` | `I` | `FUN_00424330` | open character window (`FUN_0042CBD0(2)` → mode 3) | from world or mode 3 |
| `0x4A` | `J` | `LAB_00435C8D` | `_81D764 = vk-0x37` = `0x13` | `_81D638` (debug) |
| `0x4C` | `L` | inline | toggle debug `_81D638` | **Shift** + **Ctrl** |
| `0x4D` | `M` | inline | toggle **world map** (`FUN_0042CBD0(4)` → mode 5; mode 5 → close) | from world or mode 5 |
| `0x51` | `Q` | `FUN_004242BC` | open quest-log (`FUN_0042CBD0(1)` → mode 2) | from world or mode 2 |
| `0x53` | `S` | inline | toggle `_81D564` between `0` and `_81D56C` | **Ctrl** |
| `0x54` | `T` | inline | trade toggle: flip `_81D614`; mode 7 → `FUN_00430D6C(0)`; else `FUN_0042CBD0(_81D610-1)` | gameplay |
| `0x58` | `X` | inline | `_81D764 = 0x12` | `_81D638` (debug) |
| `0x5A` | `Z` | inline | `_81D764 = 10` | `_81D638` (debug) |
| `0x6A` / `0x6B` | numpad `*` / `+` | inline | `_81D634++` (cap 2) | — |
| `0x6D` | numpad `−` | inline | `_81D634--` (floor 0) | — |
| `0xC0` | `` ` ``/`~` | inline | toggle detail of the selected unit (`FUN_0042E520`) | mode 0 only |
| everything else | — | — | **ignored (return)** | — |

Ignored ranges/keys: `0x00`–`0x08`, `0x0A`–`0x1F` (except SPACE), `0x3A`–`0x40`,
`0x4B` `K`, `0x4E` `N`, `0x50` `P`, `0x52` `R`, `0x55`–`0x57`, `0x59` `Y`,
`0x6C` (separator), and all VKs above `0xC0`.

> The literal mapping `1→slot 2`, `2→slot 0`, `3→slot 1` is **exactly what the
> binary does** (`FUN_004216DC` picks `+0x66` for arg 0, `+0x68` for arg 1,
> `+0x6A` for arg 2, and cycles for arg ≥ 3). The documented `scroll-slot`
> semantics of the three weapon fields are external; this doc only asserts the
> observed arithmetic.

### 3.2 TAB tail (selection / detail)

```c
_DAT_0081D708 = 0; _DAT_0081D670 = 0;
n = count selected (up to 10, stop at first null);
if (n < 2) {
    if (n == 0) _DAT_0081A814 = _DAT_0081D56C;              /* select leader */
    else { advance to the next occupant record; if past the end, wrap to leader }
} else {
    FUN_0043EAD0(&DAT_0081A814, 0, 0x28);                    /* clear selection (10 ptrs) */
}
FUN_0042CBD0(-1);                                            /* redraw world */
if (_DAT_0081D610 == 3) { _DAT_0081D560 = _DAT_0081A814; ... FUN_0042E520 ... return; }
if (_DAT_0081D600 == 0) return;
_DAT_0081D560 = _DAT_0081A814; FUN_0042E520(_DAT_0081A814, 0);
```

### 3.3 `0xC0` (`~`) tail

Identical "open/close detail" logic but gated on `_81D610 == 0`, and toggles
`_81D600`/`_81D630` when the selected unit is already the current detail target.

---

## 4. Handler cross-reference (what each key ultimately does)

| handler | verified behaviour |
|---|---|
| `FUN_00421554` | SPACE. The direction comes from the **first selected** unit's `+0x03` battle bit. For every selected, non-empty, living unit (`+0x04 & 0x80 == 0`, action not `3`/`9`): if entering battle and the unit's bit is clear → `+0x00 -= 6`, `+0x03 |= 4`; if leaving and the unit's bit is set → `+0x00 = min(+6,6)`, clear approach bits `+0x02 &= 0xFA` (set `0x40` if zero), `+0x03 ^= 4`; then re-idle via `FUN_00414078`/`FUN_0041435C`. Plays sound 1/0xF or 1/0xA by resulting stance. Unselected units are untouched. |
| `FUN_004216DC(arg)` | Select/cycle the active weapon slot `Entity+0x12`: arg 0 → `+0x66`, arg 1 → `+0x68`, arg 2 → `+0x6A`, arg ≥3 → advance to the next non-empty of the three, with battle-stance bookkeeping. Sounds `FUN_0042A13C(4)`, `FUN_00428DB0(0,…)`. |
| `FUN_00421458` | `A`. Sets `_DAT_0081CF69 = 10`; for every non-empty unit: zero `+0x06/+0x07`, set `+0x02 = 'D'` if `+0x0F>>24 == 1` else `'A'`, `+0x14 = 0xFF`, and if not in battle-stance enter it (`+0x00 -= 6`, `+0x03 |= 4`, `FUN_00414078`). Sound 4/7. |
| `FUN_0042246C` | `F`. For members ≥ 1 (skips the leader): `+0x02 = 0x60`, `+0x14 = 0xFF`. Sound 3/0xB. |
| `FUN_00424330` | `I`. If mode is 0 or 3: sound 6/0xD, `FUN_00428E98`; if mode 3 → close to 0 (restore `_81D600/_81D630`); else open character window `FUN_0042CBD0(2)` (→ mode 3) on `_81D560 = _81D5xxx`. |
| `FUN_004242BC` | `Q`. If mode is 0 or 2: sound 5/0xC; if mode 2 → close to 0 (`FUN_00428F94`); else open quest-log `FUN_0042CBD0(1)` (→ mode 2), clearing `_81D680`. |
| `FUN_0042E520(entity, delta)` | Panel redraw for the current interface target; also ORs `_81D61C |= 4` when `delta != 0`. |
| `FUN_00430D6C(param)` | Sets `_81D610 = 7` (inventory-detail screen) and ORs `_81D61C |= 8` when `param != 0`. |
| `FUN_0042CBD0(arg)` | Interface renderer + **mode setter**: `arg -1` → world; `0 → _81D610=1` dialogue; `1 → _81D610=2` quest-log; `2 → _81D610=3` character; `4 → _81D610=5` **world map/travel**; `3` → empty. |
| `FUN_0043EAD0` | `memset`-style clear (`FUN_0043F9D0`) — used to clear the 10-entry selection array. |

---

## 5. Global state reference

| symbol | address | role |
|---|---|---|
| `_DAT_0081D5AC` | `0x81D5AC` | top-level screen id (0 = in-game; 1..9 menus/screens) |
| `_DAT_0081D610` | `0x81D610` | interface mode (0 world / 1 dialogue / 2 quest-log / 3 character / **5 world map-travel** / 7 detail) |
| `_DAT_0081D624` | `0x81D624` | **Shift** level flag (from VK 0x10 down/up) |
| `_DAT_0081D5E4` | `0x81D5E4` | **Ctrl** level flag (from VK 0x11 down/up) |
| `_DAT_0081D638` | `0x81D638` | **debug/cheat enable** (default 0; reset on load; toggled Shift+Ctrl+L) |
| `_DAT_0081D764` | `0x81D764` | debug numeric selector target (written only when `_81D638`; also `= 1` at init `0x42B37F`). Meaning **UNKNOWN** |
| `_DAT_0081D634` | `0x81D634` | numpad `*`/`+`/`-` counter, 0..2 (reset in `FUN_004392B4`). Meaning **UNKNOWN** |
| `_DAT_0081D564` | `0x81D564` | Ctrl+S target (0 ↔ `_81D56C`); set at init to `DAT_0067C08C*0x100 + 0x78A250`. Meaning **UNKNOWN** |
| `_DAT_0081D61C` | `0x81D61C` | modal/gate flags: bit 0 gates the gameplay input block; bits 1/2/3 set by inventory(5)/`FUN_0042E520`(4)/detail(7) openers |
| `_DAT_0081D5F8` | `0x81D5F8` | "game was running" (ESC menu bookkeeping) |
| `_DAT_0081D6B8` | `0x81D6B8` | menu page selector |
| `_DAT_0081D5B0` | `0x81D5B0` | live mouse buttons (bit0 left / bit1 right) — mouse half |
| `_DAT_0081D668` / `_DAT_0081D66C` | — | ring read / write index |
| `_DAT_0081A814[0..9]` | `0x81A814` | active/party unit pointer list (`[0] = _DAT_0081D56C`) |
| `_DAT_0081D56C` | `0x81D56C` | leader (controlled hero) `Entity*` |
| `_DAT_0081D560` | `0x81D560` | current interface target entity |
| `_DAT_0081D600` / `_81D630` | — | inventory/detail selection index (column) state |

### 5.1 `_DAT_0081D638` — debug/cheat flag (evidence)

- Reset to `0` on game/asset load: `FUN_004392B4` @`0x4392CA`
  (`ev_662fb66a…`), which also resets `_81D634`.
- Toggled by **Shift+Ctrl+L** in `FUN_00435B0C` (key case `0x4C`).
- When set, `FUN_004182D4` (the "can this unit meet this item/level
  requirement" predicate) short-circuits to `1`/true for every case
  (`ev_21ccd9d4…`), i.e. **requirements are bypassed**.
- When set, the digit/letter keys write `_81D764` (see §3). `_81D764` is
  initialized to `1` at startup `0x42B37F` (`ev_ecb02cbd…`) and has no other
  writers besides the debug selector. Its consumer/meaning is **UNKNOWN**;
  `xrefs(0x81D638)` returns `0x4182E0`, `0x41C3D8`, `0x435B23..0x435D72`,
  `0x4392CA`, `0x4392D4` (`ev_34c36c2c…`); `xrefs(0x81D764)` returns
  `0x42B37F` plus the `FUN_00435B0C` writers (`ev_dba63d75…`).

---

## 6. Gates summary (why a key can be swallowed)

1. **Ring** — every key-down is stored, but only **one** is consumed per 60 ms
   frame; a full ring aliases "empty", and there is no overflow guard.
2. **Key-up** — ignored unless Shift/Ctrl.
3. **`code != 0`** — Shift/Ctrl/ESC are cleared to 0 in Stage B and never reach
   `FUN_00435B0C`.
4. **Screen** — `FUN_00435B0C` is called **only** in gameplay (`_81D5AC == 0`
   and not game-over); in menus/screens the only key handled is ESC. Non-
   gameplay screens are mouse-driven (`FUN_0042260C`).
5. **Modal gate** — the whole gameplay input block is gated by `_81D61C` bit 0
   (which is also toggled in place around the two `FUN_00435B0C` call sites).
6. **Modifiers** — Shift (`_81D624`) / Ctrl (`_81D5E4`) are level flags; several
   keys are modifier-gated (Shift+A select-all, Ctrl+S, Shift+Ctrl+L) and some
   are modifier-suppressed (a Shift-gated branch may shadow the plain action).
7. **Interface mode** — `I`/`M`/`Q`/`T` each require specific `_81D610` states.
8. **Debug flag** — all `_81D764` writes require `_81D638`.

---

## 7. Timing invariants

- Frame budget **60 ms** (`_81D5C8 = 0x3C`, set at `0x42B32F`; limiter
  `0x42B61A`; `timeGetTime`). Stage B runs once per budget.
- One ring event consumed per frame ⇒ ≤ 1 key command per 60 ms.
- `_DAT_0081D634` and `_DAT_0081D764` persist across frames until rewritten.

---

## 8. CHECK-list answers (KONUNG_SOURCE_OF_TRUTH.md §0.1)

- **CHECK 1 — Which coordinate/data domain?** `wParam` is a raw Win32
  **virtual-key code** (`0x00–0xFF`), not a scancode and not an ASCII char.
  `FUN_00435B0C` compares it directly against VK constants. The only domain
  conversions are the debug-index arithmetic `vk - 0x30` / `vk - 0x37`.
- **CHECK 2 — What exact original routine consumes it?**
  `FUN_00436214` (Stage B) pops the ring and calls **`FUN_00435B0C(vk)`**; ESC
  is consumed by Stage B itself. Ring producer is the WndProc at `0x42B6F6`
  (key-down) and `0x42B737` (key-up, Shift/Ctrl only).
- **CHECK 3 — Is there an explicit conversion before use?** No scaling,
  remapping, or fallback: the raw `wParam` is tested against literal VKs. Keys
  with no case fall through to `return` (ignored). There is **no** ASCII/
  scancode translation (`TranslateMessage`/`WM_CHAR` play no role here).
- **CHECK 4 — Is current remake behavior original / compatibility-only /
  wrong?** Compared in §10. Summary: ESC matches conceptually; SPACE,
  weapon-slot `1`/`2`/`3`, `A`, `M`, `Q`, `S` and the debug keys are
  **missing**; `F` is now the original halt, `TAB` is a stance alias; `C`/`V`/`H`/`D`/`P`/`L`/`R`/`K`
  are **remake-only**. The original binary stays authoritative.
- **CHECK 5 — Am I adding a scale/clamp/fallback that does not exist?** No.
  This document records only observed tests, or-gates, and the ring; the one
  bounded counter (`_81D634`, 0..2) and the cap on the selection scan (10) are
  literal constants in the binary, not added.

---

## 9. Cross-reference / consistency with `MOUSE_INPUT_SYSTEM.md`

The mouse document and this one describe the same pipeline and agree on:

- WndProc message table (key-down `0x42B6F6`, key-up `0x42B737`) — §1.2 there,
  §1 here.
- Ring layout (`0x81C154`/`0x81CF70`, indices `0x81D668`/`0x81D66C`, length 16).
- Stage B pop-one-event semantics and Shift/Ctrl flags `_81D624`/`_81D5E4`.
- The gameplay gate `_81D61C` bit 0 and the `FUN_00435B0C` hand-off.

**One konung.res-grounded discrepancy was found and corrected** in
`MOUSE_INPUT_SYSTEM.md`: its left-button table called code `6`
(`FUN_004242BC`) the "**inventory** window", but `FUN_004242BC` sets interface
**mode 2**, which the same document (and `FUN_0042CBD0`) define as the
**quest-log**. (Mode 5 is the world map/travel screen, not inventory/trade.) The label was corrected to
"quest-log window".

---

## 10. Remake parity (non-authoritative)

The remake does **not** use the Win32 VK dispatch. Input enters through the
browser ABI: `web/src/abi/input.rs::key_down(code)` routes a DOM
physical key (`event.code`, mapped by `www/keymap.js`) to either the frontend (`frontend/mod.rs::key_down`) or the live
game (`app/input.rs::PlayableApp::key_down`), which is a **flat `match` on a
small set of ad-hoc constants** (`web/src/constants/keys.rs`). There is **no
input ring and no one-per-frame throttle**, and Shift/Ctrl arrive as separate
`set_shift`/`set_ctrl` calls from `Event.shiftKey`/`Event.ctrlKey` (the
`_DAT_0081D624`/`_DAT_0081D5E4` level flags). There is still no `_81D61C`/debug
gating. Keys act immediately on each DOM event.

`web/src/app/input.rs::key_down` map (verbatim):

| Original (konung.res) | Remake (`app/input.rs`) | Status |
|---|---|---|
| ESC `0x1B` — screen/menu switch, close interface window, resume | ESC closes the dialogue or the character screen; otherwise `request_menu` (`abi` → `OpenMenu`; frontend ESC → `Resume`/`Quit`) | partial match (menu open/close); remake also closes its character screen |
| SPACE `0x20` - toggle battle/peace stance (`FUN_00421554`) | `toggle_selected_battle_stance` | **implemented** |
| `1`/`2`/`3` - select weapon slot (2/0/1, `FUN_004216DC`) | `select_weapon_slot(2/0/1)` | **implemented** |
| `A` - party battle-engage / Shift+A select-all | `party_engage` / `select_all_party` | **implemented** (nearest awake hostile within 12 cells; no halt) |
| `F` - party halt (`FUN_0042246C`) | `party_halt`; camera fit moved to `Home` | **implemented** |
| `I` - open character window (mode 3, `FUN_00424330`) | `I` and `C` both toggle the character screen | **implemented** |
| `M` - world map toggle (mode 5) | `toggle_map` | **implemented** |
| `Q` - quest-log (mode 2, `FUN_004242BC`) | `toggle_quest_log` | **implemented** |
| `T` — trade toggle | `teleport_debug` | **repurposed** |
| `S` (Ctrl) — toggle `_81D564` | **unbound** | **missing** |
| `0`,`4`–`9`, `B`–`H`, `J`, `X`, `Z`, `L`, `~`, numpad `*`/`+`/`-` | **unbound** | **missing** (original: debug/`_81D764`/`_81D634`) |
| TAB `0x09` - cycle party selection / open detail | stance alias (`toggle_selected_battle_stance`); portrait clicks select | **repurposed** |
| — | `C` — toggle character screen | **remake-only** (original uses `I`) |
| — | `V` — toggle camera follow | **remake-only** |
| — | `H` / Backspace — toggle graph view | **remake-only** |
| — | `D`,`P`,`L` (game debug toggles), `R` (`clear_caches`), `K` (frontend region debug) | **remake-only** debug |

Structural differences to remember when checking “parity”:

- Original Shift **is** the additive-selection modifier (`_DAT_0081D624`,
  consumed in `FUN_00424D8C` and Shift+A); the remake reproduces this for
  **mouse clicking** only (`abi::set_shift` → `select_party_member`). Ctrl
  (`_DAT_0081D5E4`) is mirrored by `abi::set_ctrl` and used by the cursor's
  party talk frame (`app/cursor.rs`).
- Original key commands are **gameplay-only** and throttled to one per 60 ms;
  the remake runs them on every DOM event regardless of the frontend/game
  split, but each handler is itself gated (e.g. `if !self.mode_graph`, or
  consumed while `dialog.is_some()`).
- The remake keeps `C`/`V`/`H` as browser aliases and maps physical
  `event.code` values through `www/keymap.js`; `TAB` is a stance alias.


---

## 11. Residual unknowns

1. **`_DAT_0081D764`** — written by the debug selector and initialized to 1;
   its consumer/meaning is unresolved (`xrefs(0x81D764)` shows only writers
   plus no obvious reader). **UNKNOWN.**
2. **`_DAT_0081D634`** — incremented/decremented by numpad `*`/`+`/`-`
   (0..2). Whether this is a camera zoom, scroll step, or party size is
   unresolved. **UNKNOWN.**
3. **`_DAT_0081D564`** — Ctrl+S toggles it between `0` and `_DAT_0081D56C`
   (leader pointer). Its role is unresolved. **UNKNOWN.**
4. **`_DAT_0081D638`** — characterized as a debug/cheat flag (bypasses equip
   requirements, enables the numeric selector); read at `0x4182E0`/`0x41C3D8`
   in gameplay code, but the full effect set is not enumerated here.
   **WORKING INTERPRETATION.**
5. **WndProc function boundary** — Ghidra exposes no function at `0x42B65C`;
   the handlers were read as raw instructions. Non-key branches of that region
   are catalogued only in `MOUSE_INPUT_SYSTEM.md`.
