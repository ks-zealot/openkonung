# Konung CD launcher — button dispatch and actions

Target: `konung.exe` from the original CD (`originalCD\konung.exe`),
`sha256 178dd38422a5d9826c1871fdb9ef2dd72fc2ca5b5a9cf6bf7344c65d767fc65e`.
All addresses are `konung.exe`-specific.

This document decodes `BGameClick` (`0x4601C8`), the single `OnClick` handler
shared by all ten bitmap buttons on `TFormStart`. The handler compares the
`Sender` (Delphi `Self`/`Sender` convention: `EAX = Self`, `EDX = Sender`) with
the form's component fields and dispatches to the corresponding action.

## 1. Handler shape

`BGameClick` is in the raw code region (Ghidra did not define a function at
`0x4601C8`). It was decoded from bytes; the method-table entry is
`0x45FAFA → 0x4601C8`.

```text
push ebp; mov ebp, esp; ...            # 0x4601C8
...
cmp esi, [ebx + 0x2C8]                 # hidden "KONUNG" branch
jne 0x4602F0
...
cmp esi, [ebx + 0x2F4]                 # BGame
cmp esi, [ebx + 0x2CC]                 # BSetup
cmp esi, [ebx + 0x2D0]                 # BIndeo
cmp esi, [ebx + 0x2D4]                 # BDirectX
cmp esi, [ebx + 0x2D8]                 # BMediaX
cmp esi, [ebx + 0x2DC]                 # BWWW
cmp esi, [ebx + 0x2E0]                 # BAuthor
cmp esi, [ebx + 0x2E4]                 # BDemo
cmp esi, [ebx + 0x2E8]                 # BInfo
```

`ebx` is the form instance; `esi` is the clicked component pointer. The chain
ends at `0x460753`; there is no default action.

## 2. Button table

| form field | button | UI label (observed) | action |
|---|---|---|---|
| `+0x2F4` | `BGame` | **ИГРА** | checks installation, then `WinExec("<Root>\\Konung.res")` |
| `+0x2CC` | `BSetup` | **Установка игры** | `WinExec("<CD>\\inst\\Install.exe")` |
| `+0x2D0` | `BIndeo` | **Установка Intel Indeo 5.06** | runs `<CD>\\Inst\\indeo506.exe` via `TBoRun` |
| `+0x2D4` | `BDirectX` | **Установка Microsoft DirectX 6.0** | runs `<CD>\\Inst\\dx6eng.exe` via `TBoRun` |
| `+0x2D8` | `BMediaX` | **Установка Microsoft DirectX Media 6.0** | runs `<CD>\\Inst\\dxmed60.exe` via `TBoRun` |
| `+0x2DC` | `BWWW` | **WWW** | `ShellExecuteA("Open", <RUN.INI Install.URL>)` |
| `+0x2E0` | `BAuthor` | **Авторы** | opens `TFAVI` and plays `<CD>\\Konung\\Video\\credits.flm` |
| `+0x2E4` | `BDemo` | **Демо** | `WinExec("<CD>\\Demo\\Demo.exe")` |
| `+0x2E8` | `BInfo` | **Информация** | `ShellExecuteA("Open", "<CD>\\Readme.txt")` |
| `+0x2C8` | hidden | (dragon/CD emblem) | secret `"KONUNG"` code path; on success runs the installed game |

`BUninst` exists in the DFM and has `OnClick = BGameClick`, but its field offset
and dispatch branch were not located; it is not visible in the observed
(non-installed) launcher. Uninstallation is provided by the installed
`Uninstall.exe` and its shortcut.

## 3. Game button (`+0x2F4`, `0x4602F0`)

`FUN_0045FE84` (`0x45FE84`,
`ev_c519fc34140fc82650fcd80d71d52708624d450a7cc6c0542ba9f61969150b5a`)
checks whether the game is installed:

```c
// read HKLM\Software\1C\Konung
RegQueryValueExA(hkey, "Version", ..., &version);
RegQueryValueExA(hkey, "Root",    ..., &root);
if (file_exists(root + "\\Client.exe")) installed = true;
```

- `RegRoot` comes from `RUN.INI [Install] RegRoot`
  (`\Software\1C\Konung`).
- `Version` must match `RUN.INI [Install] Version` (`1.0`) for the first check.
- The **installation marker** is `Root\Client.exe`.
- The game executable that the launcher runs is **`Root\Konung.res`** (the
  single-player PE), not `Client.exe`:

```c
WinExec(root + "\\Konung.res", SW_SHOW);
```

If the game is not installed, the handler shows the `game_not_set` hint
(`Игра не установлена`); no launch occurs.

## 4. Install button (`+0x2CC`, `0x460355`)

Builds `<CD>\\inst\\Install.exe`, checks it exists, and runs it with `WinExec`.
If missing, it displays `File <path> not found` (the `0x4607B4` / `0x4607C4`
strings). The installer is a separate Delphi program (`inst\Install.exe`,
562 688 bytes) documented in `KONUNG_EXE_INSTALLER.md`.

## 5. DirectX / Indeo / Media buttons (`+0x2D0/+0x2D4/+0x2D8`)

All three use the same pattern:

```c
path = "<CD>\\Inst\\<installer>.exe";
if (!file_exists(path)) { show "File <path> not found"; }
else {
    run_helper(path);          // 0x45D004 / 0x45D050 (TBoRun)
}
```

| field | path | purpose |
|---|---|---|
| `+0x2D0` | `\Inst\indeo506.exe` | Intel Indeo 5.06 codec |
| `+0x2D4` | `\Inst\dx6eng.exe` | DirectX 6.0 English |
| `+0x2D8` | `\Inst\dxmed60.exe` | DirectX Media 6.0 |

`FUN_0045FB8C` checks the installed Indeo codec by querying `VIDC.IV50`;
`FUN_0045FDD0` checks DirectX through `DSetup.dll`
(`DirectXSetupGetVersion`); `FUN_0045FC0C` checks DirectX Media/DirectShow via
`CoCreateInstance`. The hints in `FUN_00460024` reflect these results.

## 6. WWW (`+0x2DC`, `0x460578`)

```c
ShellExecuteA(hwnd, "Open", url_from_RUN_INI, NULL, NULL, SW_SHOWNORMAL);
```

The URL is `RUN.INI [Install] URL = http://www.1c.ru/konung/` (global at
`0x463880`). `0x460824` is the literal `"Open"`.

## 7. Authors (`+0x2E0`, `0x4605A5`)

1. `FUN_0045FC0C` verifies DirectShow availability.
2. Builds `<CD>\\Konung\\Video\\credits.flm` (`0x460834`).
3. Shows the `TFAVI` form and plays the FLIC/AVI credits through the
   `MP_AviM`/`DShow` units.
4. Repositions the form around the video window (`0x45F118`/`0x45F120`).

The `TFAVI` DFM resource contains `TMP_AviM` with `OnEnd = AviEnd`,
`OnKeyDown`, `OnMouseUp`; the video is `credits.flm` (13 822 842 bytes on the
CD).

## 8. Demo (`+0x2E4`, `0x46066F`)

Builds `<CD>\\Demo\\Demo.exe` and runs it with `WinExec`. `Demo.exe`
(435 712 bytes) is a separate 1C demo launcher; the CD also contains
`Demo\allods3\DEMO.AVI` and the `Demo\WAR` installer.

## 9. Info (`+0x2E8`, `0x4606EE`)

Builds `<CD>\\Readme.txt` and opens it with
`ShellExecuteA(hwnd, "Open", path, ...)`. The CD's `readme.txt` is a CP1251
document with system requirements and controls.

## 10. Hidden `KONUNG` branch (`+0x2C8`, `0x4601E3`)

The first branch is a secret/typed-code path:

```c
if (Sender == form->[0x2C8]) {
    s = first_char_of_input;           // 0x45CA20
    if (s != "KONUNG") { show message; return; }
    if (installed) WinExec(root + "\\Konung.res");
}
```

The exact UI trigger (the dragon/CD emblem at the bottom of the launcher) is
confirmed by the image mapping: `+0x2C8` loads `game.bmp`. The code compares a
string to `"KONUNG"`; the precise input buffer (`0x464A90`) and the
`sub al,0x40` check suggest a keyboard/typed sequence, but the full trigger is
not proven.

## 11. Runtime confirmation

The captured launcher window shows the labels in §2. The observed window class
is `Setup_Run`, title `Князь: Легенды Лесной страны`, size 506 × 389. No files
were written to the CD root during the run; the launcher only reads
`Konung.dat` and `RUN.INI`.

## 12. Residual unknowns

1. `BUninst` field offset/handler (not visible in the non-installed state).
2. Exact `KONUNG` secret-code input path.
3. `TBoRun` (`0x45D004`/`0x45D050`) internals: whether it waits for the child
   process or returns immediately.
4. `FUN_0045FC0C`'s exact DirectShow CLSID/IID (the GUIDs are at
   `0x463788`/`0x463928`).
