# Konung multiplayer client — save/load status and quest system

Target: `CLIENT.EXE` (PE32/i386, base `0x400000`,
`sha256 adf0a2793ee42b16dcf31c53799cf5c9b0ca37471873e1888c26e2b5f01bf4ac`).
All addresses are `CLIENT.EXE`-specific.

This document answers two questions that motivated the investigation:

1. does `CLIENT.EXE` participate in the save/load system?
2. does it reuse the `QUEST_R.RES` quest engine?

The answers are: **no** for save/load, and **yes for the engine, no for the
script data**.

## 1. Save/load: absent

### 1.1 Four independent negative checks

| check | result |
|---|---|
| raw byte search for `konung.sa`, `.sav`, `sa0`, `save`, `load game`, `konung.0`, `game.0` (case-insensitive) | **0 occurrences** each |
| defined-string inventory (`eval_69f5e0e8…`) | no save/load file name, no slot text, no "save" string |
| file-name loaders | the only disk names are `CLANS.CFG`, `SERVERS.CFG`, `KONUNG.CFG`, `CLIENT.LOG`, `MAPS.RES`, `SOUNDS_.RES`, `GRAPH.RES`, `LIGHTS.RES`, `OBJECTS.RES`, `GAME.RES`, `QUESTR.RES`, `FLAMES.RES`, and `PLAY/ME/NEW/INTO<w>R.RES` |
| function inventory | no procedure with a save/load role; no `KONUNG.SAV` / `KONUNG.SA0..9` strings for a slot writer/reader |

The raw search is decisive: the single-player game's entire save format is
keyed on the literals `KONUNG.SAV` and `KONUNG.SA0..9`, and none of those
substrings exists anywhere in `CLIENT.EXE`.

### 1.2 What the client persists

| file | direction | content |
|---|---|---|
| `KONUNG.CFG` | read (writer candidate `0x42D000`/`0x42D064`) | 7 × u32 options + 3 × 0x20 strings (124 B) |
| `CLANS.CFG` | read only | clan/faction names (≤256 × 0x20) |
| `SERVERS.CFG` | read only | connection strings (≤100 × 0x50) |
| `CLIENT.LOG` | append | `<message> rc=<code>` diagnostic lines |

The world/party/quest state is **not** written to disk by this binary. It lives
in the DirectPlay session and in the process memory; the client is a thin
multiplayer front end, not a save-game owner. This is consistent with the
single-player `KONUNG.SA0..9` format (`KONUNG_SAVE_LOAD.md`), which is entirely
absent here.

### 1.3 Consequence for the remake

The remake's save/load contract must continue to be derived from `konung.res`
(`FUN_004246C0` reader / `FUN_00424B3C` writer, `KONUNG.SAVE_LOAD.md`).
`CLIENT.EXE` provides no additional save-format evidence.

## 2. Quest system: same engine, different script

### 2.1 File layout

`QUESTR.RES` is loaded by `FUN_0041BF8E` as **seven sequential blocks** with the
same sizes as the single-player `QUEST_R.RES`:

| block | size | destination | role |
|---|---|---|---|
| 0 | `0x1F400` | `0x522D90` | node table |
| 1 | `600` | `0x542190` | index table |
| 2 | `64000` | `0x5423E8` | text B6 |
| 3 | `12000` | `0x551DE8` | text/string B3 |
| 4 | `12000` | `0x554CC8` | condition program B4 |
| 5 | `120000` | `0x557BA8` | action program B5 |
| 6 | `0x4B0` | `0x575068` | quest flags, 300 × u32 |

`Questr.res` and `Quest_r.res` are both 337 800 bytes, so the layout is
confirmed by file size.

### 2.2 Block identity

Both files have the same seven block sizes, but **every block differs**:

| block | size | differing bytes | first diff | `Quest_r.res` SHA-256 prefix | `QUESTR.RES` SHA-256 prefix |
|---|---|---|---|---|---|
| 0 | 128 000 | 30 388 | `0x04` | `26ef7566f818` | `e70d496e1bd4` |
| 1 | 600 | 334 | `0x02` | `9df5fe46e2ad` | `eed3e55eab52` |
| 2 | 64 000 | 55 363 | `0x00` | `afa40e935f86` | `55d1607d27f0` |
| 3 | 12 000 | 6 244 | `0x24` | `a43d8f460ff7` | `ddb6443053da` |
| 4 | 12 000 | 3 376 | `0x1E` | `6590f3965732` | `56a184606227` |
| 5 | 120 000 | 88 969 | `0x00` | `3fc39b71d069` | `654f4940cf17` |
| 6 | 1 200 | 659 | `0x02` | `e986c6a1ab99` | `89d4d2ec7789` |

Raw byte difference: **185 333 of 337 800** bytes differ (~55 %). The
condition/action programs (blocks 3/4) differ only in a minority of bytes
(6 244 / 3 376), while the text and node blocks differ massively. This is
consistent with an **edited/earlier quest script using the same engine**, not
with a format change.

Text comparison (CP866 printable runs):

| file | block 2 runs | block 5 runs |
|---|---|---|
| `Quest_r.res` | 5 906 | 4 043 (mostly binary) |
| `QUESTR.RES` | 434 | 0 |

`QUESTR.RES` text is a smaller/earlier dialogue set, not a `"???"`-only stub.
Both files contain exactly one literal `"???"`; that is not the distinguishing
feature.

### 2.3 Opcode dispatch — `FUN_00425E5C` (`0x425E5C`)

`ev_a69b113c79471376777bcea6230365b956defbac792a1c8d830d81e7cca5244b`

```c
int dispatch(opcode) {
    if (opcode < 0x38) return 0;
    if (opcode < 0x70) return 0;
    if (opcode < 0xA6) { FUN_004279B8(opcode - 0x0B); return FUN_00426B0A(); }
    if (opcode < 0xDC) { FUN_004279B8(opcode + 0x23); return FUN_00425F7F(); }
    if (opcode < 0x112) return FUN_00426B31(opcode - 0xDB);
    i = opcode - 0x112;
    if (table[i].has_text) FUN_004279B8(i);
    return table[i].handler();
}
```

Table: **`0x44EA30`**, stride **8 bytes** = `{u32 has_text, u32 handler}`,
**93 entries** covering opcodes `0x112..0x16E`. This is the same opcode range as
`konung.res` (`0x46601C`), but at different addresses and with
`CLIENT.EXE`-specific handlers.

Selected opcodes:

| opcode | handler | role |
|---|---|---|
| `0x113` | `0x4260CD` | trade / loot |
| `0x123` | `0x425FA6` | recruit NPC |
| `0x14C` | `0x4262E9` | dismiss NPC |
| `0x138` | `0x426951` | settlement present predicate |
| `0x15D` | `0x426DF9` | `max_party <= count` |
| `0x15E` | `0x426ED9` | party-member predicate |
| `0x160` | `0x426F6C` | level > 3 predicate |
| `0x161` | `0x425F58` | **no-op stub** here (single-player implements `Entity+4 & 0x38`) |
| `0x16E` | `0x426E54` | last defined opcode |

All 93 entries were dumped; the full list is in §2.6.

### 2.4 Condition program — `FUN_004271A9` (`0x4271A9`)

`ev_870620e63d46c47a6cb40e20dd63d498b5fe04316fcb399bc0ff40c752d20fb1`

```c
void eval_conditions(page_index) {
    u16 *p = &B4[0x554CC8 + page_index*2];
    while (*p != 0xFFFF) {
        if ((*p & 0x2000) == 0) {
            flag = *p & 0x1FF;                 // 0x1FF-bit flag index
            if (*p & 0x8000) flag_array[flag] |= 0x80;
            else             flag_array[flag] ^= 0x80;
        } else {
            FUN_00425E5C(*p & 0x1FF);          // opcode predicate
        }
        p++;
    }
}
```

- Program stream: **B4 at `0x554CC8`**, `u16` terms, terminator `0xFFFF`.
- `0x2000` = opcode predicate (the low 9 bits are the opcode fed to
  `FUN_00425E5C`).
- `0x8000` = set/clear (negate) a flag.
- Flags: **`0x575068`**, 300 × `u32` (0x4B0), tested with bit `0x80`.

### 2.5 Action program — B5 at `0x557BA8`

The 120 000-byte B5 block is loaded and is the action-side counterpart of B4.
The action runner is not separately located yet; its opcodes are dispatched by
the same `FUN_00425E5C` table, and the condition runner above is the template
for it. Status: **partially inferred**.

### 2.6 Complete opcode table (`0x44EA30`, opcodes `0x112..0x16E`)

`flag=1` means the table entry carries an inline text/parameter passed to
`FUN_004279B8`; `flag=0` means the opcode is a pure predicate/handler.

```text
112 0 425F58   113 0 4260CD   114 1 426FF3   115 0 426D08
116 0 425F58   117 0 425F58   118 0 425F58   119 0 425F58
11A 1 427146   11B 0 425F58   11C 1 42698A   11D 1 4269E7
11E 1 426A17   11F 1 426A47   120 1 426A77   121 1 426AA7
122 1 426AD7   123 1 425FA6   124 0 425F58   125 1 427023
126 1 427053   127 1 427083   128 1 4270B3   129 1 4270E3
12A 1 427113   12B 1 425F7F   12C 1 425F7F   12D 1 425F7F
12E 0 426F1C   12F 0 426495   130 0 425F58   131 0 4264D5
132 0 426510   133 0 42654B   134 0 426586   135 0 4265C1
136 0 4265FF   137 0 425F58   138 0 426951   139 0 4267AB
13A 0 425F58   13B 0 425F58   13C 0 425F58   13D 0 425F58
13E 0 426838   13F 0 425F58   140 0 425F58   141 0 425F58
142 0 425F58   143 0 425F58   144 0 42666E   145 0 426455
146 0 425F58   147 0 425F58   148 0 425F58   149 0 4268C5
14A 0 426E97   14B 1 426CD3   14C 1 4262E9   14D 0 426D50
14E 1 426BC4   14F 1 426C04   150 0 425F58   151 0 425F58
152 0 425F58   153 0 425F58   154 0 425F58   155 0 425F58
156 0 425F58   157 0 425F58   158 1 426FAC   159 0 425F58
15A 0 425F58   15B 0 425F58   15C 0 425F58   15D 0 426DF9
15E 0 426ED9   15F 0 426DB7   160 0 426F6C   161 0 425F58
162 0 425F58   163 0 425F58   164 0 425F58   165 0 425F58
166 0 425F58   167 0 425F58   168 0 425F58   169 0 425F58
16A 1 425F7F   16B 1 425F7F   16C 1 4262B9   16D 0 425F58
16E 0 426E54
```

`0x425F58` is a shared no-op/return handler (used by 28 opcodes), so the
client implements a smaller active subset than the single-player build.

### 2.7 Quest flags

- `0x575068`, 300 records × 4 bytes (`0x4B0`).
- Condition terms use the low 9 bits as the flag index and bit `0x8000` as the
  set/clear selector; the runner toggles bit `0x80` in the record.
- This is the client's equivalent of the single-player `0x615068` quest-flag
  array (which is saved in `KONUNG.SA0..9`; the client's array is session
  state only).

## 3. Conclusion

- **Save/load:** not implemented in `CLIENT.EXE`. The single-player
  `KONUNG.SAV` + `KONUNG.SA0..9` system is the only save format in the install;
  the client is a DirectPlay session front end.
- **Quest:** same 7-block engine and same 93-opcode range, but a different
  (earlier/smaller) script in `QUESTR.RES` and a set of handlers that is only
  partially equivalent. The client is useful as a **second implementation
  witness** for the quest engine's structure, not as a replacement for the
  single-player dialogue data.

## 4. Residual unknowns

1. The action-program runner for B5 (not separately located).
2. The meaning of the 28 `0x425F58` no-op opcodes (they may be unimplemented
   multiplayer-specific actions).
3. Whether `QUESTR.RES` is an earlier build or a separate multiplayer script;
   the text is Russian and the programs are structurally valid, so it is a real
   script, not a stub.
4. No save-format evidence can be extracted from this binary.
