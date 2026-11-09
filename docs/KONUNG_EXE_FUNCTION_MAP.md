# Konung CD launcher — function inventory and role map

Target: `konung.exe` from the original CD (`originalCD\konung.exe`),
`sha256 178dd38422a5d9826c1871fdb9ef2dd72fc2ca5b5a9cf6bf7344c65d767fc65e`.

Machine-readable inventories:

- `evidence/konung_exe_functions.csv` — `address,rea_name,label,kind,role,reachable,callers,callees,imports`
- `evidence/konung_exe_gates.csv` — `address,function,function_label,mnemonic,target,flag_source,flag_mnemonic,flag_operands,globals`

## 1. Method and coverage

- Provider: REA / Ghidra 12.1.4, default auto-analysis.
- Procedure inventory: `list_procedures`,
  `ev_3416f89c587e50a83f0c51e8af883e6d4f71df598524bc7283aaa4069886adde`.
- Direct-call graph from all `E8 rel32` calls in CODE; call targets resolved to
  the containing procedure.
- Roots: Delphi entry `0x461564`, `BGameClick` `0x4601C8`, `FormCreate`
  `0x4609CC`, archive loader `0x460AC8`, installed check `0x45FE84`,
  run helper `0x45D004`, credits `0x45FC0C`, registry helper `0x45C860`.
- The application code at `0x45C000–0x461800` is largely **not defined as
  Ghidra functions** (Delphi hand-written methods). It was decoded from bytes
  with Capstone and represented as `app-raw` rows in the CSV.

Totals:

| class | count |
|---|---|
| procedure entries (REA) | **2 787** |
| internal procedures | **2 385** |
| external imports | **402** |
| CSV rows (adds 6 raw pseudo-functions) | **2 793** |
| reachable procedures | **1 177** |
| VCL/runtime | **1 189** |
| VCL/runtime reachable | **1 137** |
| app raw region | **65** |
| app raw reachable | **40** |
| conditional branches | **10 828** |
| indirect jump tables | **13** |

The CSV adds six pseudo-entries for raw methods that Ghidra did not define:

| address | label |
|---|---|
| `0x45C860` | registry read helper |
| `0x45CA20` | char/key code helper |
| `0x45D004` | run helper (queue) |
| `0x45D050` | run helper (wait) |
| `0x45F118` | window position helper |
| `0x45F120` | window position helper 2 |

## 2. Core address map

### 2.1 Startup / form

| address | label | role |
|---|---|---|
| `0x461564` | PE entry | Delphi runtime entry |
| `0x4609CC` | `FormCreate` | caption, archive load, button bitmaps, status hints |
| `0x460A08` | `FormMouseMove` | hover/hint handling (raw) |
| `0x460AC8` | archive loader | `konung.zip`/`konung.dat` + `RUN.INI` |
| `0x460C08` | `RUN.INI` reader | reads `Caption/Install/Hints/message/Images` |
| `0x46087C` | button bitmap loader | 11 images → `TButtonM` |
| `0x460024` | status/hints | Indeo/DirectX/Media/game installed state |
| `0x463AA8` | `TFormStart` instance | form global |

### 2.2 Button dispatcher

| address | label | role |
|---|---|---|
| `0x4601C8` | `BGameClick` | single `OnClick` dispatcher |
| `0x4602F0` | BGame branch | installed check + `Konung.res` |
| `0x460355` | BSetup branch | `\inst\Install.exe` |
| `0x4603D4` | BIndeo branch | `\Inst\indeo506.exe` |
| `0x460460` | BDirectX branch | `\Inst\dx6eng.exe` |
| `0x4604EC` | BMediaX branch | `\Inst\dxmed60.exe` |
| `0x460578` | BWWW branch | `ShellExecute("Open", URL)` |
| `0x4605A5` | BAuthor branch | `credits.flm` |
| `0x46066F` | BDemo branch | `\Demo\Demo.exe` |
| `0x4606EE` | BInfo branch | `\Readme.txt` |
| `0x4601E3` | hidden `KONUNG` branch | secret code → game |

### 2.3 Checks and helpers

| address | label | role |
|---|---|---|
| `0x45FE84` | installed-game check | `RegRoot\Version`, `Root\Client.exe` |
| `0x45FB8C` | Indeo check | `VIDC.IV50` |
| `0x45FDD0` | DirectX check | `DSetup.dll::DirectXSetupGetVersion` |
| `0x45FC0C` | DirectShow/Media check | `CoCreateInstance` |
| `0x45D004` / `0x45D050` | `TBoRun` helpers | launch external installer |
| `0x45FC0C` (also) | credits path | `TFAVI` + `credits.flm` |
| `0x45C860` | registry helper | `RegOpenKeyExA`/`RegQueryValueExA` |

### 2.4 Companion binaries (local triage, no REA session)

| file | address class | role |
|---|---|---|
| `inst\Install.exe` | Delphi | installer (`Install.ini`, `ListFile.ini`, `setup.dat`) |
| `Uninstall.exe` | Delphi | uninstaller (`Uninstall.ini`, `ListFile.ini`) |
| `Demo\demo.exe` | Delphi | 1C demo launcher |

## 3. Comparison with the game binaries

| property | `konung.exe` (CD launcher) | `konung.res` | `CLIENT.EXE` |
|---|---|---|---|
| role | autorun launcher/installer front end | single-player game | multiplayer client |
| procedures | 2 787 | 627 | 538 |
| network | none | none | DirectPlay |
| save system | no | `KONUNG.SAV`/`SA0..9` | no |
| payload | reads `Konung.dat`/`RUN.INI` | reads game data | reads packed RES |
| launches | `Root\Konung.res` | — | — |

The launcher is the **only** binary that reads `Konung.dat` and `RUN.INI`; the
game binaries do not.

## 4. How to use the CSV

- `role` is a region/reachability classification, not a semantic audit.
- `app-raw` rows are the hand-written Delphi methods in
  `0x45C000–0x461800`; `label` is the recovered method/role.
- `external` rows are the 402 imported functions.
- The gate CSV is a mechanical branch scan. Delphi embeds data and jump tables
  in CODE; a small number of `globals` values are data bytes misread as
  addresses (e.g. ASCII `"Form"`, `"Shif"`). Treat the app-region gates
  (`0x45C000–0x461800`) as the reliable behavioural set.

## 5. Residual unknowns

1. `FormMouseMove` (`0x460A08`) raw body.
2. `BUninst` field/handler.
3. Exact `TBoRun` wait semantics.
4. Full `Install.exe`/`Uninstall.exe` method maps (only the data/UI contract was
   verified).
