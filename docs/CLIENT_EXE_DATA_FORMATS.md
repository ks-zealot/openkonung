# Konung multiplayer client — data files and configuration formats

Target: `CLIENT.EXE` (PE32/i386, base `0x400000`,
`sha256 adf0a2793ee42b16dcf31c53799cf5c9b0ca37471873e1888c26e2b5f01bf4ac`).
All addresses are `CLIENT.EXE`-specific.

The client does **not** use the single-player data model (`Game.0/1/2`,
`Konung.N`, `Lights.N`, `QUEST_R.RES`). It reads packed RES archives and a
124-byte `KONUNG.CFG`. This document records every loader and the exact byte
layout recovered from the loaders, plus the run-time archive names that are
built dynamically and therefore invisible to a string search.

## 1. File set

Literal names (defined strings, `ev_69f5e0e8481bf99f0a205cb0db0617f9679f7ff9c6468373ede93ff47a28b853`):

| file | literal address | loaded by |
|---|---|---|
| `CLANS.CFG` | `0x4400E0`, `0x440109` | `FUN_004172FF` |
| `CLIENT.LOG` | `0x4400F4` | `FUN_004169DA` |
| `SERVERS.CFG` | `0x440116` | `FUN_004172FF` |
| `KONUNG.CFG` | `0x440125`, `0x440157` | `FUN_004172FF` (read); writer candidate `0x42D000` |
| `MAPS.RES` | `0x440130` | `FUN_004176F5` |
| `SOUNDS_.RES` | `0x440184` | `FUN_0041BF8E` |
| `GRAPH.RES` | `0x4401A9` | `FUN_0041BF8E` |
| `LIGHTS.RES` | `0x4401C7` | `FUN_0041BF8E` |
| `OBJECTS.RES` | `0x4401E5` | `FUN_0041BF8E` |
| `GAME.RES` | `0x440205` | `FUN_0041BF8E` |
| `QUESTR.RES` | `0x440231` | `FUN_0041BF8E` |
| `FLAMES.RES` | `0x440263` | `FUN_0041EB35` |

Run-time-built names (pieces at `0x4402D0–0x44032B`):

| pattern | loader | purpose |
|---|---|---|
| `PLAY<width>R.RES` | `FUN_0041FDA7` (`0x41FDF0`) | player/new-hero screen |
| `ME<width>R.RES` | `FUN_0041FF3F` (`0x41FF3F`) | menu/interface |
| `NEW<width>R.RES` | `FUN_0042013A` (`0x420183`) | new-game screen |
| `INTO<width>R.RES` | `FUN_004202D2` (`0x420322`) | intro/loading screen |

`<width>` is `_DAT_007578B0` (640 for mode `0x111`, 800 for `0x114`), formatted
with `FUN_00433865` (itoa). `FUN_0041FF3F` concatenates
`"ME" @ 0x4402F4` + itoa(width) + `"R" @ 0x4402F7` + `".RES" @ 0x4402F9`.
Without `ME640R.RES` the client logs
`"ME640R.RES not found rc=16"` and exits 1 (observed locally).

## 2. Resolution tables — `FUN_0041B97D` (`0x41B97D`)

`ev_0a10bbbf8fed122d602db9d64d50651e1d8ac93b15974ccc20e1bfc89d48dcc8`

| global | 640×480 (`0x111`) | 800×600 (`0x114`) |
|---|---|---|
| width / height | `0x1E0` / `0x280` | `800` / `600` |
| screen right / bottom | `0x27F` / `0x1DF` | `799` / `599` |
| play-area left | `0x70` | `0x8C` |
| interface anchor | `0x1BC` / `0x1BD` / `0x210` | `0x22B` / `0x22C` / `0x294` |
| table selectors | `0x6EB7A8 = 7`, `0x6EB7B8 = 0x11` | `0x6EB7A8 = 9`, `0x6EB7B8 = 0x14` |

It also selects the two per-resolution table bases `0x442BA0/0x44777C`
(640) or `0x444B38/0x447EFC` (800) and calls `FUN_0042D950` (DirectDraw init).

## 3. `KONUNG.CFG` — 124 bytes

Read by `FUN_004172FF` (`0x4172FF`,
`ev_7a54fbae4c55cfea2b052db1b66db4d66a2e9941cb9a0240843f44bcc346e223`):

```c
open("KONUNG.CFG", "rb", 0x200);
read(&video_mode, 4);     // 0x757828
read(&opt1, 4);           // 0x7577CC
read(&opt2, 4);           // 0x7577E0
read(&opt3, 4);           // 0x7577F0
read(&opt4, 4);           // 0x7577F8
read(&opt5, 4);           // 0x757810
read(&opt6, 4);           // 0x757818
read(str1, 0x20);         // 0x7570B0
read(str2, 0x20);         // 0x7570D0
read(str3, 0x20);         // 0x757038
```

| offset | size | global | meaning |
|---|---|---|---|
| `0x00` | u32 | `0x757828` | video mode `0x111` / `0x114` (invalid → `0x111`) |
| `0x04` | u32 | `0x7577CC` | option flag 1 |
| `0x08` | u32 | `0x7577E0` | option flag 2 |
| `0x0C` | u32 | `0x7577F0` | option flag 3 |
| `0x10` | u32 | `0x7577F8` | option flag 4 |
| `0x14` | u32 | `0x757810` | option flag 5 |
| `0x18` | u32 | `0x757818` | option flag 6 |
| `0x1C` | 0x20 | `0x7570B0` | player name (used in packets/DirectPlay) |
| `0x3C` | 0x20 | `0x7570D0` | second string (role unresolved) |
| `0x5C` | 0x20 | `0x757038` | third string (role unresolved) |

If the file is missing, the six flags default to 1, the video mode to `0x111`,
and the three strings are zeroed. There is **no length check**; a short file
leaves the rest at BSS values. The single-player `KONUNG.CFG` is 28 bytes, so
the two formats are **not interchangeable**.

The three strings are also the source for the 0x73-byte player-state packet:
`FUN_0041701E` copies `0x7570B0` (32 B), `0x7570D0` (8 B) and `0x757038` (32 B)
into `0x6EB55C`.

## 4. `CLANS.CFG` — up to 256 × 32 bytes

`FUN_004172FF`:

```c
count = 0;
memset(0x72C26C, 0, 0x2000);          // 256 × 0x20
for each line:
    read_line(buf, 0x20);
    trim; skip leading spaces;
    if (buf[0] == 0) break;
    count++;
    if (count >= 0x100) break;
count -> 0x6EB7A4
```

- Table: `0x72C26C`, stride `0x20`, max `0x100` entries.
- Used by `FUN_004169DA` to append one line per clan to the log before dying,
  and by packet `0x0C` (clan/player name string).

## 5. `SERVERS.CFG` — up to 100 × 80 bytes

`FUN_004172FF`:

```c
count = 0;
memset(0x6C899C, 0, 8000);            // 100 × 0x50
for each line:
    read_line(buf, 0x40);
    trim; skip leading spaces;
    if (buf[0] == 0) break;
    count++;
    if (count >= 100) break;
count -> 0x757898
```

- Table: `0x6C899C`, stride `0x50`, max 100 entries.
- Each entry is a connection spec consumed by `FUN_0042D1EC`; the code uses the
  first whitespace-delimited token only (stops at a character `<= 0x20`).

## 6. `SOUNDS_.RES`

Loader: `FUN_0041BF8E` (`ev_e664a378af09b721d14e0e2300b5153a99e582375c96fdc8c70c1278f0c307cf`).

```text
read(fd, hdr, 8)                       // 8-byte header (not validated)
read(fd, index, 8000)                  // 1000 × {u32 offset,u32 size}
for i in 0..0x20:                      // 33 entries
    if index[i].size > 1:
        lseek(fd, index[i].offset + 8000, SEEK_SET);
        read(fd, buffer, index[i].size);
        pointers[i] = buffer; buffer += size;
for i in 0x20..0x37:                   // 24 more entries (3 × 8)
    if index[i].size > 1:
        lseek(fd, index[i].offset + 8000, SEEK_SET);
        read(fd, buffer, index[i].size);
```

- Data base is **file offset 8000**; offsets are relative to it.
- 57 clips are preloaded into a 3 000 000-byte heap block; pointers at
  `0x754D00`.
- File in the install: `Sounds_.res`, 13 314 264 bytes.

## 7. `GRAPH.RES`

Loader: `FUN_0041BF8E`.

```text
open;
read(fd, palette, 0x10000);            // 0x20000 bytes = 65536 u16 colors
len = filelength(fd) - 0x20000;
read(fd, buffer, 0x20000);             // first 0x20000 bytes = 8 palette blocks
for block in 0..10:                    // 11 cursor/UI blocks
    read(fd, hdr, 8);
    read(fd, buffer, hdr.size);
    pointers[block] = buffer;
    buffer += hdr.size;
read(fd, hdr, 8);
read(fd, 0x6F2BD0, 0x1000);            // fixed 0x1000 block
read(fd, buffer, hdr.size);
...
for i in 0..2:                         // 3 blocks of 0x4D28
    read(fd, src, 0x4D28);
    fix up the in-place offset at src+4;
    read(fd, buffer, size);
for i in 0..0x17:                      // 24 blocks of 0x4D28
    read(fd, src, 0x4D28);
    src[0] = table_0x44885C[i];
    read(fd, buffer, size);
```

- Palette: `0x20000` bytes (65 536 × u16, converted to 5-5-5 at load).
- Then 8 blocks of `0x200` (`0x6F6310`).
- 11 cursor/interface blocks stored at `0x754BAC`.
- One `0x1000` block at `0x6F2BD0`.
- 27 blocks of `0x4D28` (3 + 24) — the client's interface screens.
- File in the install: `Graph.res`, 14 505 371 bytes.

## 8. `LIGHTS.RES`

Loader: `FUN_0041BF8E`.

```text
read(fd, 0x500010, 0x1000);            // global light table
read(fd, 0x575518, 0x0AC0);            // 2752 bytes
for i in 0..0x12:                      // 19 blocks
    read(fd, 0x501010 + i*0x1C80, 0x1C80);
```

- 4096 + 2752 + 19 × 7296 = **145 088** bytes read.
- File in the install: `Lights.res`, 145 472 bytes; **384 bytes remain unread**
  (trailing data or a skipped header — unresolved).

## 9. `OBJECTS.RES`

Loader: `FUN_0041BF8E`.

```text
read(fd, hdr, 8);
read(fd, table, 0x1000);               // 512 × {u32 offset,u32 size}
for i in 0..0x1FF while table[i].offset < 0x80000000:
    table[i].offset += 0x1008;         // data base
    lseek(fd, table[i].offset, SEEK_SET);
    if i < 0x1E:
        table[i].offset += 0x3228;
        table[i].size  -= 0x3228;
        read(fd, 0x66A8EC + i*0x3228, 0x3228);
    else:
        table[i].offset += 0x100;
        table[i].size  -= 0x100;
        read(fd, 0x73092C + i*0x100, 0x100);
```

- Header 8 bytes + 512-entry table (0x1000) at `0x1008`.
- First 30 object templates: `0x3228` bytes each, at `0x66A8EC`.
- Remaining templates: `0x100` bytes each, at `0x73092C`.
- File in the install: `Objects.res`, 52 522 058 bytes.

## 10. `GAME.RES` — packed game archive

Loader: `FUN_0041BF8E`; the `GAME.RES` string is referenced at `0x41D4F2`
(`ev_6dd405fc6ac8b6e1f7d0bf519d2e7f80612e6b1411f4d643e4caabdbc2d6ddd6`).

```text
read(fd, 0x5C772C, 0x20000);
read(fd, 0x5E772C, 0x00A00);
read(fd, 0x577FD8, 0x20D00);
lseek(fd, 0x0109A, SEEK_CUR);          // skip 4250 bytes
read(fd, 0x6CA8DC, 0x14000);
read(fd, 0x6E81FC, 0x02C2B);
```

| file offset | size | destination | meaning |
|---|---|---|---|
| `0x00000` | `0x20000` | `0x5C772C` | item class/subtype table |
| `0x20000` | `0x00A00` | `0x5E772C` | 80 × 0x20 records (player/clan slots) |
| `0x20A00` | `0x20D00` | `0x577FD8` | world-object records, 21 map slots × 100 × 0x40 (`0x1900`/map) |
| `0x41700` | `0x0109A` | — | skipped |
| `0x4279A` | `0x14000` | `0x6CA8DC` | 320 × 0x100 entity/hero templates |
| `0x5679A` | `0x02C2B` | `0x6E81FC` | 3 × 0xEB9 settlement/group records |

Total **0x593C5 = 365 509 bytes**, exactly the install's `Game.res`. The
`lseek` uses `SEEK_CUR` (origin `1`), so the gap is intentional, not an overlap.

After loading, the client copies 0x100 bytes from `0x6CA8DC` to `0x757580` and
builds the per-player name pointer table at `0x5B82E0` from the 0x50 records at
`0x5E772C`.

## 11. `MAPS.RES` — packed map records

Loader: `FUN_004176F5` (`0x4176F5`,
`ev_835bd275d636d1ee0c279996f213c3c8b5248485c71bb63190ab83f056aeedfc`).

```c
lseek(fd, (map_id - 1) * 0x2F120, SEEK_SET);
read(fd, 0x6EB7D0, 0x6400);
read(fd, 0x5A64D8, 0xC800);
read(fd, 0x750D2C, 16000);
read(fd, 0x6DF55C, 36000);
read(fd, 0x722B10, 0x3200);
read(fd, 0x716310, 0xC800);
```

| within record | size | destination | meaning |
|---|---|---|---|
| `0x00000` | `0x6400` | `0x6EB7D0` | base terrain (same size as the single-player terrain block) |
| `0x06400` | `0xC800` | `0x5A64D8` | raw collision, 320 × 160 bytes |
| `0x12C00` | `0x3E80` | `0x750D2C` | sprite/placement table, 1000 × 0x10 entries |
| `0x16A80` | `0x8CA0` | `0x6DF55C` | spawn/prop table, 1000 × 0x24 records |
| `0x1F320` | `0x3200` | `0x722B10` | placement/aux table |
| `0x22520` | `0xC800` | `0x716310` | secondary collision/visibility block |
| **end** | `0x2F120` | — | 192 800 bytes per map |

`Maps.res` is 3 856 000 bytes = **20 × 0x2F120**, so the client ships 20 maps.
The raw collision block is converted to the runtime `u16` grid at `0x599CD8`
(320 × 160): `0xFF` → `0x0FFF`; low nibble `0xF` → `(b << 8) | 0x0FFF`; else
`b << 8`.

World objects are 100 × 0x40 at `0x577FD8 + map_id*0x1900`; active types are
marked in the collision grid with bit `0x20` (same convention as the
single-player build).

## 12. `QUESTR.RES` — 7 sequential blocks

Loaded by `FUN_0041BF8E` into fixed buffers (exact same block sizes as the real
`QUEST_R.RES`):

| block | size | destination | role (from the quest engine) |
|---|---|---|---|
| 0 | `0x1F400` (128 000) | `0x522D90` | node/entry table |
| 1 | `600` | `0x542190` | small index table |
| 2 | `64000` | `0x5423E8` | text block B6 |
| 3 | `12000` | `0x551DE8` | text/string table B3 |
| 4 | `12000` | `0x554CC8` | condition program B4 |
| 5 | `120000` | `0x557BA8` | action program B5 |
| 6 | `0x4B0` (1200) | `0x575068` | quest flags, 300 × u32 |

`Questr.res` in the install is 337 800 bytes = sum of the seven blocks. It is a
**different script** from `Quest_r.res` (see `CLIENT_EXE_SAVE_LOAD_QUEST.md`):
blocks 3/4 are identical, blocks 0/1/2/5/6 differ.

## 13. `FLAMES.RES`

Loader: `FUN_0041EB35` (`0x41EB35`,
`ev_3c2afb50daadb9f93233b80357f76ee39d067f601d3a3c7f73b1b0739e5bd29d`).

```text
open(FLAMES.RES)
for i in 0..0x49:                       // 74 frames
    pointers[i] = 0x754BD8 + i*4;
    read(fd, hdr, 8);
    read(fd, hdr.data, hdr.size);
    if (option 0x7577CC) decompress(0x41D828, frame);
    else if (video_depth == 0x10) convert16(0x41DB30, frame);
    pointers[i] += size;
```

- 74 frames; pointer table `0x754BD8`.
- Optional RLE decompression (`0x41D828`) gated by option `0x7577CC`, or 16-bit
  conversion (`0x41DB30`) when the video depth is 16.
- File in the install: `Flames.res`, 1 533 070 bytes.

## 14. Runtime buffers relevant to the formats

| buffer | address | size | content |
|---|---|---|---|
| terrain | `0x6EB7D0` | `0x6400` | current map terrain |
| raw collision | `0x5A64D8` | `0xC800` | current map collision bytes |
| runtime collision | `0x599CD8` | `0x140*0xA0*2` | u16 collision grid |
| object templates 30 | `0x66A8EC` | `30*0x3228` | OBJECTS.RES |
| object templates rest | `0x73092C` | `N*0x100` | OBJECTS.RES |
| entity/hero templates | `0x6CA8DC` | `0x14000` | GAME.RES (320 × 0x100) |
| settlement/group records | `0x6E81FC` | `0x2C2B` | GAME.RES (3 × 0xEB9) |
| world objects | `0x577FD8` + map*0x1900 | `0x1900`/map | 100 × 0x40 |
| spawn/prop table | `0x6DF55C` | `0x8CA0` | 1000 × 0x24 (MAPS.RES) |
| sprite/placement table | `0x750D2C` | `0x3E80` | 1000 × 0x10 (MAPS.RES) |
| player slots | `0x5E812C` | `10*0x74` | scoreboard/team list |
| player name table | `0x5B2ED8` | `N*0x20` | packet `0x0C` |
| clan names | `0x72C26C` | `0x2000` | CLANS.CFG |
| server list | `0x6C899C` | `0x1F40` | SERVERS.CFG |
| sound clips | `0x754D00` | `0x3A*4` pointers | SOUNDS_.RES |
| interface screens | `0x754BAC` | `11*4` pointers | GRAPH.RES |
| flame frames | `0x754BD8` | `0x4A*4` pointers | FLAMES.RES |

## 15. Verified vs unresolved

**Verified**

- `KONUNG.CFG` = 124 bytes (7 × u32 + 3 × 0x20), not the 28-byte single-player
  format.
- `CLANS.CFG` = ≤256 × 0x20; `SERVERS.CFG` = ≤100 × 0x50.
- `MAPS.RES` = 20 × 0x2F120 and the six sub-block sizes.
- `GAME.RES` = 365 509 bytes and the exact offset table including the
  `SEEK_CUR 0x109A` gap.
- `QUESTR.RES` = 7 blocks / 337 800 bytes.
- `FLAMES.RES` = 74 frames.
- Dynamic archive names `PLAY/ME/NEW/INTO<width>R.RES`.
- `ME640R.RES` is required at startup (runtime observation).

**Unresolved**

1. Exact block layouts inside the `PLAY`, `NEW` and `INTO` archives (the four
   loaders are located; only `ME` and the packed data archives are fully
   traced).
2. The roles of the second and third `KONUNG.CFG` strings.
3. `CLANS.CFG`/`SERVERS.CFG` exact line grammar (line-oriented, 32/64-byte
   buffers, trimmed of leading spaces).
4. `LIGHTS.RES` trailing 384 bytes.
5. `OBJECTS.RES` table validation (offsets are trusted; no bounds check).
