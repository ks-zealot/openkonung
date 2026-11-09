# Konung CD launcher — branch and gate catalogue

Target: `konung.exe` from the original CD (`originalCD\konung.exe`),
`sha256 178dd38422a5d9826c1871fdb9ef2dd72fc2ca5b5a9cf6bf7344c65d767fc65e`.

This document summarises `evidence/konung_exe_gates.csv` and calls out the
gates that select launcher behaviour. Addresses are `konung.exe`-specific.

## 1. Coverage

| item | count |
|---|---|
| conditional branches | **10 828** |
| indirect jump tables | **13** |
| functions containing gates | 1 100+ |
| reachable procedures | 1 177 |

The high count is normal for a Delphi/VCL application: most branches are in the
VCL runtime (string handling, streaming, controls, graphics). The
application-specific gates are concentrated in `0x45C000–0x461800`.

> Caveat: Delphi embeds data, method tables and bitmap blobs inside CODE. A
> linear scan can misread data bytes as instructions, so a few `globals` values
> in the CSV are ASCII fragments (`"Form"`, `"Shif"`, …). The app-region rows
> and the hand-decoded gates below are reliable.

## 2. Startup / form gates

| address | condition | effect |
|---|---|---|
| `0x4609CC` | `FormCreate` entry | caption + archive + bitmaps + hints |
| `0x460AC8` | `file_exists("\konung.zip")` | extract `konung.zip`, else try `konung.dat` |
| `0x460AC8` | `file_exists("\konung.dat")` | extract `konung.dat` and load `run.ini` |
| `0x46087C` | 11 image loads | assign `background/left/right/game` bitmaps |
| `0x460024` | Indeo check | `set_yes` vs `set_not` hint |
| `0x460024` | DirectX check | DirectX hint |
| `0x460024` | DirectShow check | DirectX Media hint |
| `0x460024` | installed check | `run_game` vs `game_not_set`; enable/disable |

## 3. `BGameClick` dispatcher (`0x4601C8`)

The handler is a sender-pointer comparison chain. Each `cmp esi,[ebx+off]`
selects one button; `jne` advances to the next.

| address | compare | target on match | action |
|---|---|---|---|
| `0x4601E3` | `[ebx+0x2C8]` | `0x4601E3` body | hidden `KONUNG` secret |
| `0x4602F0` | `[ebx+0x2F4]` | `0x4602F0` body | **BGame** |
| `0x460355` | `[ebx+0x2CC]` | `0x460355` body | **BSetup** |
| `0x4603D4` | `[ebx+0x2D0]` | `0x4603D4` body | **BIndeo** |
| `0x460460` | `[ebx+0x2D4]` | `0x460460` body | **BDirectX** |
| `0x4604EC` | `[ebx+0x2D8]` | `0x4604EC` body | **BMediaX** |
| `0x460578` | `[ebx+0x2DC]` | `0x460578` body | **BWWW** |
| `0x4605A5` | `[ebx+0x2E0]` | `0x4605A5` body | **BAuthor** |
| `0x46066F` | `[ebx+0x2E4]` | `0x46066F` body | **BDemo** |
| `0x4606EE` | `[ebx+0x2E8]` | `0x4606EE` body | **BInfo** |

Each branch then tests file existence (`FUN_00408004`) before launching; a
missing file produces `"File <path> not found"` (`0x4607B4`/`0x4607C4`).

### 3.1 BGame branch (`0x4602F0`)

```text
FUN_0045FE84()                  // installed?
if (!installed) -> hint "game_not_set"
else {
    path = Root + "\Konung.res";
    if (!file_exists(path)) -> hint
    else WinExec(path, SW_SHOW);
}
```

### 3.2 BSetup branch (`0x460355`)

```text
path = "<CD>\inst\Install.exe";
if (!file_exists(path)) -> "File ... not found"
else WinExec(path, SW_SHOW);
```

### 3.3 Indeo / DirectX / Media branches

```text
path = "<CD>\Inst\{indeo506|dx6eng|dxmed60}.exe";
if (!file_exists(path)) -> "File ... not found"
else run_helper(path);          // 0x45D004/0x45D050
```

### 3.4 WWW / Author / Demo / Info

| branch | condition | action |
|---|---|---|
| `0x460578` | URL non-empty | `ShellExecuteA("Open", URL)` |
| `0x4605A5` | DirectShow available | play `credits.flm` |
| `0x46066F` | `Demo\Demo.exe` exists | `WinExec` |
| `0x4606EE` | `Readme.txt` exists | `ShellExecuteA("Open", path)` |

### 3.5 Hidden `KONUNG` branch (`0x4601E3`)

```text
s = first_char_of_input;
if (s != "KONUNG") -> message; return
if (!installed) -> message; return
WinExec(Root + "\Konung.res");
```

The comparison uses `sub al,0x40` and a string at `0x46075C` (`"KONUNG"`). The
exact typed/keyboard trigger is only partially decoded.

## 4. Installed-game gate (`0x45FE84`)

`ev_c519fc34140fc82650fcd80d71d52708624d450a7cc6c0542ba9f61969150b5a`

```text
RegQueryValueExA(HKLM\Software\1C\Konung, "Version", &version);
if (version == RUN.INI Version) {
    RegQueryValueExA(HKLM\Software\1C\Konung, "Root", &root);
    if (file_exists(root + "\Client.exe")) installed = true;
}
```

- `Version` is `1.0` from `RUN.INI`.
- `Root` is the install directory written by `Install.exe`.
- `Client.exe` is the installation marker.
- The game executable is `Root\Konung.res`.

## 5. Environment checks

| check | function | condition |
|---|---|---|
| Indeo 5.06 | `0x45FB8C` | codec `VIDC.IV50` present |
| DirectX | `0x45FDD0` | `DSetup.dll::DirectXSetupGetVersion` version in `0x40001..0x40005` |
| DirectX Media / DirectShow | `0x45FC0C` | `CoCreateInstance` succeeds |
| Game installed | `0x45FE84` | registry `Root\Client.exe` exists |

These results drive the `set_yes` / `set_not` / `run_game` / `game_not_set`
hints and the enabled state of the install buttons.

## 6. File / path gates

| literal | address | use |
|---|---|---|
| `\konung.zip` | `0x45FFD8` | preferred launcher archive |
| `\konung.dat` | `0x460BFC` | fallback launcher archive |
| `run.ini` | `0x460BEC` | launcher configuration |
| `\inst\Install.exe` | `0x460798` | installer |
| `\Konung.res` | `0x46076C` | game executable |
| `\Uninstall.exe` | `0x460780` | uninstaller (installed root) |
| `\Inst\indeo506.exe` | `0x4607D8` | Indeo installer |
| `\Inst\dx6eng.exe` | `0x4607F4` | DirectX installer |
| `\Inst\dxmed60.exe` | `0x460810` | DirectX Media installer |
| `\Demo\Demo.exe` | `0x460858` | demo launcher |
| `\Readme.txt` | `0x460870` | info |
| `\Konung\Video\credits.flm` | `0x460834` | credits |

## 7. How to read a gate row

```text
address=0x4601E3 function=0x4601C8 mnemonic=jne target=0x4602F0
flag_source=0x4601E3 flag_mnemonic=cmp flag_operands=esi, dword ptr [ebx + 0x2C8]
globals=0x... 
```

This means: if the clicked component is **not** `[ebx+0x2C8]`, continue to the
BGame comparison at `0x4602F0`.

## 8. Verification notes

- The app-region gates were decoded from bytes because Ghidra did not define
  functions there.
- The method table at `0x45FAFA–0x45FB3E` is the authoritative mapping of
  `BGameClick`/`FormCreate`/`FormMouseMove`.
- The installer/uninstaller are separate Delphi binaries; their gates were not
  enumerated branch-by-branch, only their data/UI contracts.
