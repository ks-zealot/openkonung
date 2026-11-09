# Konung CD launcher — startup, Delphi form and RUN.INI

Target: `konung.exe` from the original CD (`originalCD\konung.exe`), PE32/i386,
image base `0x400000`, 482 304 bytes,
`sha256 178dd38422a5d9826c1871fdb9ef2dd72fc2ca5b5a9cf6bf7344c65d767fc65e`.

This is the **CD autorun launcher**, not the game engine. It is a Borland
Delphi application (`TPF0` / `TFormStart`, VCL, `PACKAGEINFO`). All addresses are
`konung.exe`-specific.

Companion documents: `KONUNG_EXE_LAUNCHER.md`, `KONUNG_EXE_INSTALLER.md`,
`KONUNG_EXE_CD_LAYOUT.md`, `KONUNG_EXE_FUNCTION_MAP.md`,
`KONUNG_EXE_BRANCHES.md`.

## 1. Identity and layout

| property | value |
|---|---|
| SHA-256 | `178dd38422a5d9826c1871fdb9ef2dd72fc2ca5b5a9cf6bf7344c65d767fc65e` |
| size | 482 304 |
| machine | i386 (`0x14C`), PE32 |
| image base | `0x400000` |
| entry | `0x461564` |
| sections | Headers `0x400000`; CODE `0x401000–0x461800`; DATA `0x462000`; BSS `0x464000`; .idata `0x465000`; .tls `0x468000`; .rdata `0x469000`; .reloc `0x46A000`; .rsrc `0x471000–0x47B000` |
| procedures | 2 787 entries (2 385 internal + 402 external imports) |
| strings | 1 365 defined string objects |
| imports | 11 DLLs, 402 functions (kernel32, user32, gdi32, advapi32, ole32, oleaut32, comctl32, comdlg32, shell32, mpr, quartz) |

Evidence: `ev_a895fafd067ba5dcbd4979f59fc75d0ff0aca853c42324986d76fedc615f2772`
(overview), `ev_3416f89c587e50a83f0c51e8af883e6d4f71df598524bc7283aaa4069886adde`
(procedure inventory).

## 2. Delphi resources

The `.rsrc` directory contains:

| type | name | size | meaning |
|---|---|---|---|
| `RCDATA` (10) | `DVCLAL` | 16 | Delphi version signature |
| `RCDATA` (10) | `PACKAGEINFO` | 960 | unit list |
| `RCDATA` (10) | `TFAVI` | 472 | AVI credits form |
| `RCDATA` (10) | `TFORMSTART` | 15 980 | main form (binary DFM) |
| `STRINGTABLE` (6) | `#269`, `#4084..#4096` | — | VCL resource strings |
| icons/cursors/bitmaps | `MAINICON`, `#1..#7`, `#2/*` | — | VCL defaults |

`PACKAGEINFO` lists the application units: `wkonung`, `MP_AviM`, `GMMSystem`,
`DShow`, `DSound`, `DDraw`, `VCLUnZip`, `kpZipObj`, `BoRun`, `BoFileList`,
`BoMousServ`, `BoJustOne`, `Registry`, `IniFiles`, `Reg_ini`, `WinShell`,
`ShellAPI`, `BoShlobj`, `BoVerInfo`, `XZipFunction`, `GameSetupConst`,
`ButtonM`, `BmpPanel`, `AVarKonung`, `UAvI`.

The unit list already identifies the three functional layers:

1. `VCLUnZip` + `kpZipObj` + `XZipFunction` — ZIP extraction.
2. `BoRun` + `WinShell` + `ShellAPI` + `BoShlobj` — process launch/shell.
3. `Registry` + `IniFiles` + `Reg_ini` — registry/INI configuration.

## 3. Main form `TFormStart`

`TFORMSTART` is a binary Delphi form stream (`TPF0`). Parsed properties:

| property | value |
|---|---|
| class | `TFormStart` |
| name | `FormStart` |
| border | `bsSingle` (fixed-size, system menu + minimize) |
| position | `poScreenCenter` |
| font | `1C_Sans_Serif`, `RUSSIAN_CHARSET` |
| events | `OnCreate = FormCreate`, `OnMouseMove = FormMouseMove` |
| runtime size | 506 × 389 (observed window rect `478,262–984,651`) |

Components:

| component | class | event | role |
|---|---|---|---|
| `BmpPanel1` | `TBmpPanel` | — | background image panel |
| `BGame` | `TButtonM` | `BGameClick` | **ИГРА** |
| `BSetup` | `TButtonM` | `BGameClick` | **Установка игры** |
| `BIndeo` | `TButtonM` | `BGameClick` | **Установка Intel Indeo 5.06** |
| `BDirectX` | `TButtonM` | `BGameClick` | **Установка Microsoft DirectX 6.0** |
| `BMediaX` | `TButtonM` | `BGameClick` | **Установка Microsoft DirectX Media 6.0** |
| `BWWW` | `TButtonM` | `BGameClick` | **WWW** |
| `BAuthor` | `TButtonM` | `BGameClick` | **Авторы** |
| `BDemo` | `TButtonM` | `BGameClick` | **Демо** |
| `BInfo` | `TButtonM` | `BGameClick` | **Информация** |
| `BUninst` | `TButtonM` | `BGameClick` | uninstall button (not shown in the observed, non-installed state) |
| `BoRun1` | `TBoRun` | — | process runner |
| `BoFile1` | `TBoFile` | — | file helper |
| `Timer1` | `TTimer` | — | UI timer |
| `BoMousServ1` | `TBoMousServ` | — | mouse/hover helper |

The labels are bitmap captions, not DFM `Caption` text; the running launcher
(see §7) confirms them.

The published method table in the code/data region maps the event names:

| method | address |
|---|---|
| `BGameClick` | `0x4601C8` |
| `FormCreate` | `0x4609CC` |
| `FormMouseMove` | `0x460A08` |

Evidence: method table at `0x45FAFA–0x45FB3E`; `BGameClick` pointer stored at
`0x45FAFA`.

## 4. FormCreate (`0x4609CC`)

`ev_e24bf24b31f19720e3745f4d904f871ca4fa161360848c4327ee732134c8505a`

```c
void FormCreate(TFormStart *self) {
    FUN_00444C64(PTR_DAT_00463970, PTR_DAT_0046396C);  // set Caption/Title
    FUN_0042922C(self, PTR_DAT_00463884);              // VCL form init
    FUN_0046087C();                                    // load button bitmaps
    FUN_00460024(self);                                // status/hints
}
```

### 4.1 Archive load — `FUN_00460AC8` (`0x460AC8`)

`ev_17f76fa59ad71679bfdccdc2de0e2499e4bca21f35f072bbf69e3fd6693d5b5e`

```c
if (file_exists("\\konung.zip")) {
    extract("\\konung.zip");
    load_ini(form, "\\konung.zip", "run.ini");
} else if (file_exists("\\konung.dat")) {
    extract("\\konung.dat");
    load_ini(form, "\\konung.dat", "run.ini");
}
```

- The launcher prefers `konung.zip`, then `konung.dat`.
- Both are **streaming ZIP archives** (local headers, no central directory).
- `FUN_0045BCD0` is the extract/INI-load helper; the `VCLUnZip`/`kpZipObj`
  units and the `TBoRun`/`TBoFile` components implement it.
- The archive contains the launcher background/button bitmaps and `RUN.INI`.

### 4.2 Button bitmaps — `FUN_0046087C` (`0x46087C`)

`ev_1992ebc52291ee5d197f18818e6d3c43fcef3ac32f6d8b54c7137e8ee77dcd9f`

Loads the 11 images from the extracted archive into the `TButtonM` controls:

| form field offset | image key (RUN.INI `[Images]`) | component |
|---|---|---|
| `+0x2C4` (panel) | `background` | `BmpPanel1` |
| `+0x2CC` | `left1` | `BSetup` |
| `+0x2F4` | `left1alt` | `BGame` |
| `+0x2D0` | `left2` | `BIndeo` |
| `+0x2D4` | `left3` | `BDirectX` |
| `+0x2D8` | `left4` | `BMediaX` |
| `+0x2DC` | `right1` | `BWWW` |
| `+0x2E0` | `right2` | `BAuthor` |
| `+0x2E4` | `right3` | `BDemo` |
| `+0x2E8` | `right4` | `BInfo` |
| `+0x2C8` | `game` | hidden `KONUNG`/secret button |

This table is the authoritative **button-field → image** mapping and is used by
`BGameClick` (see `KONUNG_EXE_LAUNCHER.md`).

### 4.3 Status and hints — `FUN_00460024` (`0x460024`)

`ev_a24bfe553ab13bc000be6b3e5daab219e7b6ab8098408cb02225ea8c206c17f2`

Updates the hint labels and enables/disables the install buttons:

- `FUN_0045FB8C` — checks the **Indeo 5.0 codec** (`VIDC.IV50`); sets
  `set_yes`/`set_not`.
- `FUN_0045FDD0` — checks **DirectX** through `DSetup.dll` /
  `DirectXSetupGetVersion`; sets the DirectX hint.
- `FUN_0045FC0C` — checks **DirectX Media/DirectShow** via
  `CoCreateInstance`; sets the media hint.
- `FUN_0045FE84` — checks the **game installation** (see launcher doc).
- The game button hint becomes `run_game` when installed, `game_not_set`
  otherwise; install buttons are enabled/disabled accordingly.

## 5. `RUN.INI`

`RUN.INI` is inside `Konung.dat` (and would be inside `konung.zip`). It is a
CP1251 INI file:

```ini
[Caption]
Title=Князь
Caption=Князь: Легенды Лесной страны

[Install]
Version=1.0
RegRoot=\Software\1C\Konung
URL=http://www.1c.ru/konung/

[Hints]
run_setup=Запуск программы установки
set_yes=Установлен
set_not=Рекомендуется установить
Look_www=Посмотреть WWW-страницу игры
authors=Авторы игры
setup_demo=Установить демонстрационные версии других игр 1С
info=Последняя информация по игре
run_uninstall=Запуск программы удаления
run_game=Запуск игры
game_not_set=Игра не установлена

[message]
message0=Вставьте CD-ROM "Князь: Легенды Лесной страны"
message1=На этом компьютере программной части "Князь: Легенды  Лесной страны" не обнаружено.

[Images]
background=background.bmp
left1=left1.bmp
left1alt=left1alt.bmp
left2=left2.bmp
left3=left3.bmp
left4=left4.bmp
right1=right1.bmp
right2=right2.bmp
right3=right3.bmp
right4=right4.bmp
game=game.bmp
```

`FormCreate` reads these keys into global strings (`PTR_DAT_004636xx` /
`0x4637xx`); `BGameClick` and the status labels use them.

## 6. `Konung.dat` archive

`Konung.dat` (217 636 bytes) is a streaming ZIP with **12 entries**:

| entry | compressed | raw |
|---|---|---|
| `background.bmp` | 108 826 | 181 078 |
| `right4.bmp` | 8 407 | 14 110 |
| `right3.bmp` | 8 090 | 14 110 |
| `right2.bmp` | 8 276 | 14 110 |
| `right1.bmp` | 8 377 | 14 110 |
| `left4.bmp` | 10 987 | 14 110 |
| `left3.bmp` | 10 595 | 14 110 |
| `left2.bmp` | 10 429 | 14 110 |
| `left1alt.bmp` | 9 407 | 14 110 |
| `left1.bmp` | 9 385 | 14 110 |
| `game.bmp` | 23 164 | 35 422 |
| `RUN.INI` | 525 | 890 |

There is **no central directory** (`PK\x01\x02` present but no `PK\x05\x06`);
the launcher/installer use the VCLUnZip streaming reader.

## 7. Runtime corroboration (local, not REA-captured)

The unmodified launcher was started from the CD root. It created a visible
window of class **`Setup_Run`**, title `Князь: Легенды Лесной страны`, rect
`478,262–984,651` (506 × 389). The captured window shows:

- top: **ИГРА**
- bottom left: **Установка игры**, **Установка Intel Indeo 5.06**,
  **Установка Microsoft DirectX 6.0**, **Установка Microsoft DirectX Media 6.0**
- bottom right: **WWW**, **Авторы**, **Демо**, **Информация**
- center: the Konung logo/stone artwork; the dragon/CD emblem is the hidden
  `KONUNG` button field (`+0x2C8`).

No files were created in the CD root during the run. The launcher reads
`Konung.dat` and `run.ini`; it does not modify the CD.

## 8. Global anchors

| item | address / value |
|---|---|
| PE entry | `0x461564` |
| main form | `TFormStart`, `TFORMSTART` resource |
| `FormCreate` | `0x4609CC` |
| `BGameClick` | `0x4601C8` |
| `FormMouseMove` | `0x460A08` |
| archive loader | `0x460AC8` |
| RUN.INI reader | `0x460C08` |
| button bitmap loader | `0x46087C` |
| status/hints | `0x460024` |
| installed check | `0x45FE84` |
| form object | `0x463AA8` (TFormStart instance pointer) |
| `RUN.INI` | inside `Konung.dat` |
| player/game registry root | `\Software\1C\Konung` |

## 9. Residual unknowns

1. Exact `FUN_0045BCD0` extraction implementation (VCLUnZip call sequence).
2. `FormMouseMove` (`0x460A08`) is a raw region; its hover/hint behaviour is
   not fully decoded.
3. `BUninst` field offset and handler branch (see launcher doc).
4. The hidden `KONUNG` branch is a secret code path; its exact trigger is
   partially decoded.
