# Konung — save, load, configuration and state persistence

Target: `konung.res` (PE32/i386, image base `0x400000`,
`sha256 d698c4f231489a372cc25212ab2819720a36d7ce5307a045db684135cf679810`).
All addresses are virtual addresses in `konung.res`.

Everything in this file was recovered statically. No game process was launched
and no save was created (static-only scope). Field semantics that the code does
not establish are marked **UNKNOWN**.

## 1. File locations

All game files are opened as `path = <data-root> + <literal>`:

- `<data-root>` is the string at `0x81CD30`. It is initialised to an empty string
  and is the same prefix used for `GAME_x`, `QUEST_R.RES`, `FLAMES.RES`,
  `LIGHTS.<loc>` and `KONUNG.<loc>`.
- Because the CD-drive probe (`0x81CC2C`) can leave the CD root empty, the
  effective paths are normally **relative to the process working directory**.

| file | literal | evidence |
|---|---|---|
| configuration | `KONUNG.CFG` @ `0x4502D8` | `ev_9c7da740ee368a1a6202e032edf8b16c84682c1e96ccda2eb21a18c654b444ab` |
| slot-name table | `KONUNG.SAV` @ `0x45020E` / `0x4506AE` | `ev_4ccac98b8699c1eb95ae5f5f13c71b77b61874d67e8c8a47f80b7aea312ee02c` |
| save snapshots | `KONUNG.SA0`..`KONUNG.SA9` @ `0x450219` | `ev_b37503220ddb799cb3598ecf5eb618e0d1e3ce97440586fa4fa2c4031cc0b918` |

The install directory does **not** ship these files; they are created at runtime
by the save screen. The `konung.res` executable itself contains no embedded
save.

## 2. `KONUNG.CFG`

### 2.1 Size and layout

**28 bytes**, seven little-endian `u32` values, no header and no checksum.

| offset | global after read | role | evidence |
|---|---|---|---|
| `+0x00` | `_DAT_0081D644` | video mode: `0x111` = 640×480, `0x114` = 800×600 | WinMain `0x42B3F6`–`0x42B4B2` |
| `+0x04` | `_DAT_0081D5D4` | Прозрачность (transparency) | options screen `0x437980` |
| `+0x08` | `_DAT_0081D5EC` | Смена дня и ночи (day/night cycle) | `0x4379DE` |
| `+0x0C` | `_DAT_0081D604` | Световые эффекты (light effects) | `0x437A0D` |
| `+0x10` | `_DAT_0081D60C` | Музыка (music) | `0x437A3C` |
| `+0x14` | `_DAT_0081D620` | Тени (shadows) | `0x4379AF` |
| `+0x18` | `_DAT_0081D62C` | Звук (sound) | `0x437A6B` |

Labels were read from the original settings page (`menu640_001.png`,
`frontend_640_settings.png`) and mapped to the six toggle handlers in control
order (640/800 are controls 0/1, then the six checkboxes top-to-bottom).
Verified split: `_DAT_0081D60C` gates **music** (`FUN_004296B0` /
`FUN_00429714`, MCI `mciSendCommandA`), while `_DAT_0081D62C` gates
**sound effects** (`FUN_0042A13C`).

### 2.2 Read (startup)

At WinMain `0x42B3D6`:

```c
fd = open("KONUNG.CFG", O_RDONLY|O_BINARY);   // CRT wrapper 0x43ECA7, mode 0x200
if (fd != -1) {
    read(fd, &_DAT_0081D644, 4);
    read(fd, &_DAT_0081D5D4, 4);
    read(fd, &_DAT_0081D5EC, 4);
    read(fd, &_DAT_0081D604, 4);
    read(fd, &_DAT_0081D60C, 4);
    read(fd, &_DAT_0081D620, 4);
    read(fd, &_DAT_0081D62C, 4);
    close(fd);
    if (video_mode != 0x111 && video_mode != 0x114)
        video_mode = 0x111;                    // force 640x480 on bad value
} else {
    _DAT_0081D62C = _DAT_0081D620 = _DAT_0081D60C = _DAT_0081D604 =
    _DAT_0081D5EC = _DAT_0081D5D4 = 1;         // all option flags default to 1
    video_mode = 0x111;
}
FUN_0042781C(video_mode);                       // direct-draw/resolution setup
```

The reads are **not length-checked**; a short file leaves later globals at their
BSS values.

### 2.3 Write (options screen)

Options screen state 6, `0x437B44`–`0x437C24`:

```c
fd = open("KONUNG.CFG", O_RDWR|O_CREAT|O_TRUNC|O_BINARY); // wrapper 0x43F16F, flags 0x180
if (fd != -1) {
    write(fd, &0x831C58, 4);        // options working copy (0x111 / 0x114)
    write(fd, &_DAT_0081D5D4, 4);
    write(fd, &_DAT_0081D5EC, 4);
    write(fd, &_DAT_0081D604, 4);
    write(fd, &_DAT_0081D60C, 4);
    write(fd, &_DAT_0081D620, 4);
    write(fd, &_DAT_0081D62C, 4);
    close(fd);
}
```

The writer opens with the CRT flags observed at `0x43F16F` → `0x43ECC9`
(`CreateFileA` with create/truncate semantics). Failure is silent; there is no
error message and no retry.

## 3. `KONUNG.SAV` — slot-name table

### 3.1 Size and layout

**500 bytes = 10 slots × 50 bytes.** Each slot is a NUL-terminated CP866 string
(the code uses `strcpy`/`strcat`/`strlen`, so it is C-string data, not fixed
binary fields).

- Loaded at startup by `FUN_004392B4` (`0x439680`–`0x4396F6`):
  `read(fd, 0x653518, 500)`.
- If the file is missing, all ten slots are filled from the literal
  `"Пустая строка"` @ `0x4506B9` and `_DAT_0081D678 = -1` (no selection).
- If the file exists, `_DAT_0081D678 = 0` initially.
- The save screen writes the whole 500-byte table back (`FUN_00424B3C`, first
  block) after copying the edited name for the selected slot.
- The displayed name is composed at `0x4368C1`–`0x4369BA`:
  `default location name` + `" "` + a formatted date/time string, then the
  formatter output is truncated at the `:` before the seconds field.
- Exact format (`FUN_0043F57C` → `FUN_0043F5ED`, an `asctime` clone):
  `Www Mmm dd hh:mm:ss yyyy\n`; leading zero of the day is replaced by a space.
  The save-name loop strips the newline, seconds and year, then removes the
  remaining colon, leaving `Www Mmm dd hh:mm`.
- Example: `Родники Tue Oct 21 20:15`.

### 3.2 Default names

`0x45B394` is the **56-entry location-name table**, CP866, stride `0x28`
(40 bytes), indices `0..55`. The save screen builds the selected name from
`0x45B394 + (current_location - 1) * 0x28` and appends a space
(`0x4368D0`–`0x4368FB`). Examples:

- entry 0 (used for location 1): `Родники`
- entry 1: `Укромное место`
- entry 3: `Старый лагерь`

Full dump: `evidence/konung_location_names.csv`.

The table is a **source of a default display name**; the actual slot text is
the buffer `0x81CE34`, which is copied into `KONUNG.SAV[slot]` when saving.

### 3.3 Slot selection

`_DAT_0081D678` is the selected slot, `0..9`:

- `-1` = nothing selected / no `KONUNG.SAV`;
- `0..9` = selected row in the 10-row list.

The snapshot filename is `KONUNG.SA` + `('0' + _DAT_0081D678)`, i.e.
`KONUNG.SA0` through `KONUNG.SA9`. This is implemented by
`strlen(path); path[last] += _DAT_0081D678` at `0x424BE8` (save) and
`0x42475F` (load).

## 4. `KONUNG.SA0`..`SA9` — snapshot format

### 4.1 Exact layout

The writer `FUN_00424B3C` and reader `FUN_004246C0` use the same sequential
order. All values are little-endian. The file has no magic, version, checksum or
length prefix.

| offset | size | source/target global | meaning |
|---|---|---|---|
| `0x000000` | `u32` | `_DAT_0081D5EC` | option flag 2 |
| `0x000004` | `u32` | `_DAT_0081D604` | option flag 3 |
| `0x000008` | `u32` | `_DAT_0081D640` | world/frame tick counter |
| `0x00000C` | `u32` | leader entity index | `(leader_ptr - 0x78A250) / 0x100` |
| `0x000010` | `u32` | `_DAT_0081D6A0` | active projectile/effect count (`0..100`) |
| `0x000014` | `u32` | `_DAT_0081D6A8` | active ground-marker count |
| `0x000018` | `0x4B0` | `0x615068` | quest flags, 300 × `u32` |
| `0x0004C8` | `0xC80` | `0x80B0D0` | projectile/effect pool, 100 × `0x20` |
| `0x001148` | `0x20000` | `0x659B0C` | item class/subtype table (GAME_x block) |
| `0x021148` | `0x3200` | `0x785BB4` | 200 × 0x40 ground-marker records |
| `0x024348` | `0x1900` | `0x67C08C` | 400 × 0x10 actor name/record table |
| `0x025C48` | `0x7D00` | `0x77DEB4` | 500 × 0x40 WorldObject records |
| `0x02D948` | `0x7D000` | `0x78A250` | 2000 × 0x100 Entity records |
| `0x0AA948` | `0x5E24` | `0x8149F0` | 20 × 0x4B5 settlement records |
| `0x0B076C` | `0x38` | `0x81CF80` | 56 discovered world-map location flags |
| **end** | `0x0B07A4` | — | **722 852 bytes** |

The four leading fields (16 bytes) plus the leader record are read first; the
remaining blocks are read in order from offset `0x10`.

### 4.2 Writer `FUN_00424B3C` (`0x424B3C`–`0x424D8B`)

Evidence: `ev_ff1ece81f670f72c2287b52109fdfde0829704de445d9f957f6b36c44513a75b`.

```text
1. open("<data-root>KONUNG.SAV", create/truncate) and write 0x653518[500].
2. open("<data-root>KONUNG.SA" + slot, create/truncate).
3. write header:
     _DAT_0081D5EC, _DAT_0081D604, _DAT_0081D640, leader_index.
4. write _DAT_0081D6A0, _DAT_0081D6A8.
5. write the 12 blocks listed above (quest flags .. 0x81CF80).
6. close.
```

`leader_index` is computed from the pointer difference
`(_DAT_0081D56C - 0x78A250) / 0x100`, with the compiler's signed-division
correction. The leader is always `Entity[0]` in the shipped party data, but the
format stores the index explicitly.

No return value is produced; open failures are silently ignored.
The **first** open (`KONUNG.SAV`) and the snapshot open are independent: a
failure of one does not abort the other.

### 4.3 Reader `FUN_004246C0` (`0x4246C0`–`0x424B38`)

Evidence: `ev_82b6a0cf7fba776a112c2b13d3a2e85b330457931c2e606ee08e048556bf90e8`.

```text
1. Clear: _81D560, _81D5BC, _81D5B8, _81D630, _81D624, _81D610,
          _81D600, _81D5E4, _81D568.
2. path = <data-root> + "KONUNG.SA" + slot; open read-only binary.
3. If open fails -> return 0 (save screen returns to menu page 4).
4. Read 16-byte header; set leader = 0x78A250 + index*0x100;
   clear the 10-entry selection array and set selection[0] = leader.
5. FUN_0043A75C()   -> load FLAMES.RES.
6. Save current file position; seek to
      0x10 + 0x2D938 + index*0x100   (= 0x2D948 + index*0x100)
   and read 0x100 bytes into 0x81D45C (startup leader copy);
   seek back.
7. FUN_0043A870()   -> load GAME_x + QUEST_R.RES from the install data.
8. Read _81D6A0, _81D6A8, then the 12 blocks in the same order as the writer.
9. close.
10. Derive _DAT_0081D55C = &actor_record[leader[+5] * 0x10] and
    _DAT_0081D6C4 = (actor_record[leader[+5]].field >> 24)  (location id).
11. FUN_0042C384()  -> party/camera refresh.
12. FUN_0043ADF0(1) -> map load in "load" mode.
13. If map load fails -> message "Map not found!" and return 0.
14. For each party entity: mark its cell in the collision/u16 map.
    For each active ground marker: set cell bit 0x20.
15. Center camera on the leader, refresh render, FUN_0042CBD0(-1), return 1.
```

The seek target is exactly the Entity block base `0x2D948`; this independently
confirms the layout table.

### 4.4 Map load mode: new game vs load

`FUN_0043ADF0(mode)` (`ev_9c1620dc0ac95e2e8dea2ee291a29d914ab0a795d98e5ea8deca3e9ce6a64dfd`)
loads `LIGHTS.<loc>` and `KONUNG.<loc>` and rebuilds the runtime world.

- **mode 0** (new game, from state 8): clears `Group+0x1D` for object/enemy
  groups (`+0x1E & 0x40`), places the initial population (`FUN_00411DE4`),
  spawns population growth, rebuilds the `0x785BB4` ground-marker list and
  re-projects WorldObjects.
- **mode 1** (load, from `FUN_004246C0`): skips the group-activity clear, does
  **not** re-place the population and does **not** rebuild `0x785BB4` /
  WorldObjects from `KONUNG.<loc>`; those come from the snapshot. It still
  loads terrain/collision/GRAPH/props, activates types and refreshes entity
  anchors (`FUN_00414078`).

This is why the snapshot must carry `0x77DEB4`, `0x785BB4`, `0x78A250` and
`0x81D6A8`: mode 1 deliberately does not regenerate them.

## 5. Screen integration

The save/load screens are top-level states of `_DAT_0081D5AC` (see
`KONUNG_STATE_MACHINE.md`):

| state | screen | open | confirm | failure/back |
|---|---|---|---|---|
| `4` | load | `FUN_004304F4(0)` draws 10 slots; `_DAT_0081D678` from hit test | `FUN_004246C0()`; success → state `0` | → state `7`, menu page `4` |
| `5` | save | `FUN_004304F4(1)`; name buffer `0x81CE34` for selected slot | copy `0x81CE34` → `KONUNG.SAV[slot]`; `FUN_00424B3C()`; → state `0` | — |

`FUN_004304F4` (`ev_e273464e81dbbc1b32be53d4937e8535fa7537a458d60fc989756f582ef86cbe`)
uses the per-resolution 10-row rectangle table at `0x46241C` (stride `0x20`,
indexed by `_DAT_0081D644 * 0x10`).

## 6. Verified and unknown

**Verified**

- `KONUNG.CFG` = 7 × u32, 28 bytes, same order read/write.
- Option labels: Прозрачность, Смена дня и ночи, Световые эффекты, Музыка,
  Тени, Звук (plus 640×480 / 800×600 and OK/Cancel).
- `KONUNG.SAV` = 10 × 50-byte CP866 slot names, 500 bytes.
- `KONUNG.SA0`..`SA9` = 0xB07A4 bytes, exact block order and sizes.
- `0x80B0D0` = 100 × 0x20 projectile/effect pool; `_81D6A0` = active count.
- `0x81CF80` = 56 discovered world-map location flags.
- Save name = location default + space + `Www Mmm dd hh:mm` (asctime truncated).
- Load mode 1 does not regenerate population/WorldObjects/ground markers.
- Slot filename is `SA` + digit `0..9`; no other slot encoding exists.

**UNKNOWN**

1. Whether save/load ever validate a version or reject a truncated file — the
   code has no such check; a short file simply leaves globals partially
   overwritten.

## 7. Remake browser save (wasm-only)

The browser build has no filesystem, so it does not write `KONUNG.SA0..9`.
`web/src/save.rs` defines a versioned native payload instead:

- header `OPKS` + `u16` version + raw length, then **bincode + deflate**;
- the original state classes are preserved, including all **300 x u32 B7
  quest-flag slots** (`+0x02` journal line included), item instances,
  `Group[200]`, `Entity[2000]`, `Settlement[20]` (+ merchant stocks),
  `WorldObject` sources, active ground markers, per-map population/motion,
  party membership, the 56 discovered-map flags, world tick, projectiles and
  the combat RNG;
- `www/app.js` stores one compressed base64 payload per slot in
  `localStorage` (`openkonung.save.<slot>`), with the 10 display names in
  `openkonung.sav`;
- the original `SAVE{width}R.RES` 10-slot rectangles (`0x46241C`, 640 at
  `+0`, 800 at `+0x10`) are used for hit-testing; loading restores population,
  markers and WorldObjects directly (original load mode `1`) and never
  re-places `(0,0)` actors.

Quest progress therefore round-trips exactly as in the original: the saved B7
block is restored after `QUEST_R.RES` is loaded, and all dependent tables
(group/entity/settlement/item/world-object state) are restored with it.

## 8. Legacy interop (implemented, `web/src/legacy_save.rs`)

- `LegacySnapshot` parses and writes the exact section 4.1 layout
  (`0xB07A4`), including the 100 x `0x20` projectile pool and the two runtime
  pools (`0x3200` markers, `0x7D000` entities).
- `export_legacy_snapshot` copies the `Game.N` blocks (groups/entities/world
  objects/settlements) and patches every field the remake models, so bytes the
  remake does not interpret keep their `Game.N` values. The projectile pool has
  no recovered field map and is exported empty (count `0`).
- `import_legacy_snapshot` rebuilds `Group[200]`, `Entity[2000]`,
  `Settlement[20]` (+ trade stocks), `WorldObject[500]`, the active marker pool
  and the leader from the snapshot, then applies them in original load mode `1`
  (no growth, no actor re-placement). The leader index comes from the header and
  is written back on export.
- `KONUNG.SAV` names are parsed/written as 10 x 50 CP866 C strings
  (`parse_slot_names` / `write_slot_names`).
- Browser wiring: `I` imports a `KONUNG.SAn` file, `E` exports the live game as
  one, `Shift+I`/`Shift+E` do the same for `KONUNG.SAV`; the ABI is
  `legacy_export_size/ptr`, `legacy_import`, `legacy_names_parse`,
  `legacy_names_text_ptr/len`, `legacy_names_build`.
