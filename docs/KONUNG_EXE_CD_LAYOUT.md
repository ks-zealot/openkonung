# Konung original CD — layout, payload and music

CD root: `originalCD\` (copied from the original disc).

## 1. Root

| item | size | role |
|---|---|---|
| `Autorun.inf` | 53 | `open = Konung.exe`, icon `Konung.exe,0` |
| `konung.exe` | 482 304 | Delphi autorun **launcher** |
| `Konung.dat` | 217 636 | launcher UI archive (streaming ZIP: bitmaps + `RUN.INI`) |
| `readme.txt` | 2 159 | CP1251 system requirements/controls |
| `inst\` | — | installer + payload |
| `Konung\` | — | CD-side game data and videos |
| `Demo\` | — | 1C demo launcher and demo payloads |

`Autorun.inf`:

```ini
[autorun]
open = Konung.exe
icon = Konung.exe,0
```

## 2. `inst\` — installer and payload

| file | size | role |
|---|---|---|
| `Install.exe` | 562 688 | Delphi installer |
| `Install.dat` | 1 200 124 | installer UI archive (11-entry streaming ZIP) |
| `setup.dat` | 75 543 320 | full game payload (119-entry streaming ZIP) |
| `Install.dat`/`setup.dat` | — | `.zip` variants would be preferred if present |
| `dx6eng.exe` | 3 760 920 | DirectX 6.0 English installer |
| `dxmed60.exe` | 4 678 944 | DirectX Media 6.0 installer |
| `indeo506.exe` | 1 809 135 | Intel Indeo 5.06 codec installer |

`setup.dat` raw payload total: **142 835 552 bytes**; compressed file:
**75 543 320 bytes**. It contains the complete installed game, including:

- `CLIENT.EXE` (939 008) — multiplayer client;
- `konung.res` (1 586 688) — single-player game PE (the launcher runs this);
- `Game.0`, `Game.1`, `Game.2` (709 822 each) — save/state templates;
- `Game.res` (365 509), `Objects.res` (52 522 058), `Sounds_.res` (13 314 264),
  `Maps.res` (3 856 000), `Quest_r.res` / `QUESTR.RES` (337 800),
  `FLAMES.RES`, `GRAPH.RES`, `LIGHTS.RES`;
- all `Konung.1..50`, `LIGHTS.1..50`;
- `INTO/INTR/ME/MENU/NEW/NEWH/PLAY/SAVE` archives for 640/800;
- `playavi.dll`, `Uninstall.exe`, `Uninstall.ini`, `konung.ico`.

The installer copies these to the target folder and registers
`HKLM\Software\1C\Konung` with `Root`/`Version` (see
`KONUNG_EXE_INSTALLER.md`).

## 3. `Konung\` — CD-side data

| file | size |
|---|---|
| `GRAPH.RES` | 14 505 349 |
| `LIGHTS.RES` | 145 472 |
| `Video\credits.flm` | 13 822 842 |
| `Video\poster.flm` | 130 182 090 |
| `Video\win.flm` | 82 695 810 |
| `Video\gameover.flm` | 3 555 636 |
| `Video\defeat*.flm` | ~2.6–3.7 MB each (16 files) |
| `Video\PLAYAVI.DLL` | 28 160 |
| `Video\playavi2.dll` | 316 416 |

The launcher's **Авторы** button plays `Konung\Video\credits.flm`; the game
itself uses `POSTER.FLM`, `WIN.FLM`, `GAMEOVER.FLM` and `DEFEAT<loc>.FLM` from
the installed copy (`playavi.dll`).

Note: the CD `GRAPH.RES` is 14 505 349 bytes, while the install payload's
`GRAPH.RES` is 14 505 371 bytes — a 22-byte difference. The installed copy is
authoritative for the game; the CD-side copy is the launcher's data set.

## 4. `Demo\` — bundled 1C demos

| item | size |
|---|---|
| `demo.exe` | 435 712 (generic 1C demo launcher) |
| `PlayCD.ini` / `PlayCD.res` | 441 / 173 378 |
| `allods3\DEMO.AVI` | 33 139 530 |
| `allods3\PlayCD.res` | 12 |
| `WAR\*` | InstallShield demo (`DATA1.CAB`, `SETUP.EXE`, …) |

The Konung launcher's **Демо** button runs `Demo\Demo.exe`.

## 5. Music / CD audio

The CD is a mixed-mode disc: track 1 is the data track, and tracks **2–11**
are **Red Book CD-DA audio tracks**. They are not files in the ISO9660
filesystem, so Explorer does not show them (and a mounted ISO may expose only
the data track). The repository contains the extracted rips:

```
music\track02.ogg   2 159 474
music\track03.ogg   2 139 986
music\track04.ogg   2 095 120
music\track05.ogg   2 084 081
music\track06.ogg   2 170 775
music\track07.ogg   2 344 696
music\track08.ogg   1 891 264
music\track09.ogg   1 933 650
music\track10.ogg   2 138 386
music\track11.ogg   2 474 209
```

- 10 tracks, matching audio tracks 2–11.
- The launcher does not play them; the single-player `konung.res` uses MCI CD
  audio (`winmm.dll::mciSendCommandA`).
- The installed game's `SOUNDS.RES`/`Sounds_.res` is a separate archive of
  digital sound effects/voices, not the music.

## 6. Streaming ZIP note

`Konung.dat`, `Install.dat` and `setup.dat` all start with `PK\x03\x04` and
contain local file headers, but they have **no End-of-Central-Directory
record** (`PK\x05\x06` absent) and only a partial/central-directory-like tail.
They are written by a streaming ZIP library (the Delphi `VCLUnZip`/`kpZipObj`
units) and must be read sequentially, not with a standard ZIP reader that
requires the central directory.

This is why the payloads are named `.dat` rather than `.zip` and why a normal
unzip tool reports "not a valid zip file".

## 7. Verified vs unresolved

**Verified**

- CD root, `Autorun.inf`, `readme.txt`, launcher, `inst`, `Konung`, `Demo`.
- `setup.dat` = 119-entry streaming ZIP with the full game payload.
- `Konung.dat` = 12-entry launcher UI ZIP with `RUN.INI`.
- `Install.dat` = 11-entry installer UI ZIP.
- Audio tracks 2–11 correspond to the 10 `music/track*.ogg` rips.

**Unresolved**

1. Exact CD-DA track lengths/ISRCs (not read from the physical disc here).
2. The 22-byte `GRAPH.RES` difference between CD and install payload.
3. Whether the original CD contains additional hidden/system files outside the
   ISO9660 data track.
