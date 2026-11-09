# Konung multiplayer client — startup, WinMain, WndProc and frame loop

Target: `CLIENT.EXE` (PE32/i386, image base `0x400000`,
`sha256 adf0a2793ee42b16dcf31c53799cf5c9b0ca37471873e1888c26e2b5f01bf4ac`,
939 008 bytes, 538 procedure entries, 174 defined strings).

All addresses are **`CLIENT.EXE`-specific**. They must not be reused for
`konung.res`; the two binaries share the CRT/stub convention and the Win32
skeleton but have different code and data layouts.

This document is new and does not modify `KONUNG_STARTUP_MAIN.md` (which
describes the single-player `konung.res`). Companion documents:
`CLIENT_EXE_STATE_MACHINE.md`, `CLIENT_EXE_NETWORK.md`,
`CLIENT_EXE_DATA_FORMATS.md`, `CLIENT_EXE_SAVE_LOAD_QUEST.md`,
`CLIENT_EXE_FUNCTION_MAP.md`, `CLIENT_EXE_BRANCHES.md`.

## 1. Identity and layout

| property | value |
|---|---|
| SHA-256 | `adf0a2793ee42b16dcf31c53799cf5c9b0ca37471873e1888c26e2b5f01bf4ac` |
| size | 939 008 |
| machine | i386 (`0x14C`), PE32 |
| image base | `0x400000` |
| segments | Headers `0x400000–0x400400`; BEGTEXT `0x410000–0x439800`; DGROUP `0x440000–0x4F6400`; .bss `0x500000–0x5E5400`; .idata `0x780000–0x780A00`; .reloc `0x790000`; .rsrc `0x7A0000` |
| procedures | 538 entries; 462 internal, 72 external imports, 4 thunks |
| strings | 174 defined string objects |

Evidence: `ev_de15569ea6831285c6e5252f2af1f5aa5940d148db4e80db2a5c26295444a5a3`
(overview), `ev_c696c9a58980c2efb4b49bb4b9a55b58e45e8968b843c44827abcc87799b0d6b`
(procedure inventory).

## 2. Entry chain (verified)

| step | address | evidence | meaning |
|---|---|---|---|
| PE entry | `0x433A72` | `ev_8eaa3e02126a140fda8855e211b2983022249a2a3a202c2c32ac9537da3ed6cc` | 15-byte CRT stub |
| stub body | `0x433A72` | same | `MOV [0x7786D4], 0x42EBC8; JMP 0x435910` |
| CRT startup | `0x435910` | procedure inventory | calls the function pointer at `0x7786D4` |
| WinMain | `0x42EBC8` | `ev_04926cd001b6e5f31a277394ecc70d4eb615cf6a85151c2e8a67b76693361635`, `ev_d74a90969b09099693a3b24a3e59d94819f9998dd89d93d7ac00c7c8dd21b193` | not a Ghidra function; decoded from bytes |
| WndProc | `0x42EF70` | `ev_8bae803f38572d80712aa2683061a75b707d2ee59757fda537123f9ed875e646` | not a Ghidra function; decoded from bytes |

The entry stub writes the WinMain pointer to `0x7786D4` and jumps to the CRT
startup. The CRT startup calls that pointer and exits with its return value.
This is the same pattern as `konung.res` (`0x43F4B4` → `0x44016C` →
`0x42B1A8`), with different addresses.

## 3. WinMain (`0x42EBC8`, stdcall, `ret 0x10`)

Reconstructed order of operations:

1. Build `WNDCLASS` on the stack:
   - `style = 3` (`CS_HREDRAW | CS_VREDRAW`);
   - `lpfnWndProc = 0x42EF70`;
   - `cbClsExtra = cbWndExtra = 0`;
   - `hInstance = WinMain.hInstance`;
   - `hIcon = LoadIconA(hInstance, "GAME_ICON" @ 0x440657)`;
   - `hCursor = LoadCursorA(NULL, 0x7F00)` (`IDC_ARROW`);
   - `hbrBackground = GetStockObject(4)` (`BLACK_BRUSH`);
   - `lpszMenuName = NULL`, `lpszClassName = "Konung_Client" @ 0x44ED50`.
2. `RegisterClassA(&wc)`.
3. `CreateWindowExA(8, "Konung_Client", "Konung_Client", 0x80000000,
   0, 0, GetSystemMetrics(0), GetSystemMetrics(1), 0, 0, hInstance, 0)`.
   `8 = WS_EX_TOPMOST`, `0x80000000 = WS_POPUP` — borderless full-desktop popup.
   The handle is stored at `0x77861C`; `NULL` aborts with return 0.
4. `ShowWindow(hwnd, nCmdShow)`, `UpdateWindow(hwnd)`, `SetFocus(hwnd)`.
5. `FUN_004306ED()` — CRT/startup helper (analogue of `FUN_0043BFE4` in the
   single-player build; exact CRT identity not needed by the game).
6. `_DAT_007786B4 = 1` (application active), `_DAT_007577AC = 0` (mouse-state
   dword).
7. Build the **data root**:
   - `strcpy(0x6EAEB0, "A:\\" @ 0x440661)`;
   - try drive letters `'A'..'Z'`, calling `GetDriveTypeA` (thunk `0x4395F0`);
     accept the first drive whose type is `5` (`DRIVE_CDROM`);
   - if none, write `0` to `0x6EAEB0`;
   - zero `0x6EAF40` and copy it over `0x6EAEB0`.
   Net effect: the effective path prefix `0x6EAF40` is the **empty string**, so
   every loader opens relative paths from the process working directory. The
   CD probe result is deliberately discarded in this build.
8. Initialise global state:
   - `_DAT_006EB774 = 9` — initial screen state (menu/preload transient);
   - `_DAT_006EB0EC = 0`;
   - `_DAT_006EB778 = 0x3C` — frame budget **60 ms**;
   - `_DAT_006EB780 = 0` — quit-request flag;
   - `_DAT_006EB788 = 0`, `_DAT_006EB7CE = 0xFF`, `_DAT_006EB798 = 0`,
     `_DAT_006EB794 = 0`, `_DAT_006EB7BC = 0`, `_DAT_006EB7C8 = 0`,
     `_DAT_006EB770 = 0`, `_DAT_00757780 = 0`;
   - `FUN_0042CEC0()` → `_DAT_007577D0` (time/random seed initialisation);
   - `_DAT_007577E4 = 0`, `_DAT_007577D8 = 0`, `_DAT_00757804 = 1`,
     `_DAT_00778624 = 0`, `_DAT_00778630 = 0` (DirectPlay interface pointer),
     `_DAT_0077862C/_628 = 0`.
9. `strncpy(0x7785FC, 0x440B7C, 0x10)` — copy the 16-byte DirectPlay
   **application GUID** used later by the lobby.
10. `GetUserNameA(0x77857C, &size)` (size initialised from
    `strlen(0x77857C)`); if it fails, `strcpy(0x77857C, "Unknown" @ 0x440665)`.
    `0x77857C` is the **player name** later handed to DirectPlay.
11. `FUN_004172FF()` — load `CLANS.CFG`, `SERVERS.CFG`, `KONUNG.CFG`
    (see `CLIENT_EXE_DATA_FORMATS.md`).
12. `FUN_0041B97D(video_mode)` — set the 640/800 coordinate tables and
    initialise DirectDraw (`FUN_0042D950`). Failure destroys the window and
    returns 0.
13. `FUN_0042DB60(hwnd)` — DirectSound init. Failure destroys the window and
    returns 0.
14. `FUN_0041BF8E()` — asset loader (`SOUNDS_.RES`, `GRAPH.RES`,
    `LIGHTS.RES`, `OBJECTS.RES`, `GAME.RES`, `QUESTR.RES`). Failure destroys the
    window and returns 0.
15. `_DAT_007786B5 = 1` (running), `last_tick = timeGetTime()`.
16. Enter the message/frame loop.
17. On exit return 1.

## 4. Message/frame loop (`0x42EE??–0x42EF6F`)

```text
while (_DAT_007786B5) {                     // running
    if (PeekMessageA(&msg, 0, 0, 0, PM_REMOVE)) {
        TranslateMessage(&msg);
        DispatchMessageA(&msg);
    } else if (_DAT_007786B4) {             // application active
        now = timeGetTime();
        if (now - last_tick >= _DAT_006EB778) {   // 60 ms
            last_tick = now;
            SendMessageA(hwnd, 0x0400, 0, 0);     // WM_USER: one frame
        }
    }
}
```

Differences from the single-player loop:

- There is **no movie mode**. The client does not import `PlayAVI.dll` and has
  no blocking `GetMessage` branch / `WM_USER+0x0D` movie pump.
- The loop is otherwise identical: event-driven, one `WM_USER 0x400` frame per
  60 ms while the window is active, no timer thread and no `Sleep`.

## 5. WndProc (`0x42EF70`, stdcall, `ret 0x10`)

Ghidra defines no function at `0x42EF70`; the body was decoded from bytes.
Argument layout: `hwnd=[ebp+0x14]`, `msg=[ebp+0x18]`, `wParam=[ebp+0x1C]`,
`lParam=[ebp+0x20]`.

### 5.1 Quit pre-check

If `_DAT_006EB780` (quit requested) is non-zero:

```text
_DAT_007786B5 = 0;                    // running = false
_DAT_006EB780 = (signed)_DAT_007786B2 >> 24;   // restore/clear quit flag
DestroyWindow(hwnd);
return 0;
```

### 5.2 Message table (complete range tree at `0x42F265`)

| message | value | handler @ | effect |
|---|---|---|---|
| `WM_CREATE` | `0x0001` | `0x42F014` | return 0 |
| `WM_DESTROY` | `0x0002` | `0x42F019` | `FUN_0042D788()`, `FUN_0042D69C()`, `FUN_0042D174()`; `PostQuitMessage(0)` |
| `WM_PAINT` | `0x000F` | `0x42F182` | `BeginPaint`; if `_DAT_006EB774 == 1` → `FUN_00420A96(-1)` + `FUN_0042E19C()`; `EndPaint` |
| `WM_ACTIVATEAPP` | `0x001C` | `0x42EFCB` | `_DAT_007786B4 = (byte)wParam` |
| `WM_SETCURSOR` | `0x0020` | `0x42F219` | `SetCursor(0)`, return 1 |
| `WM_KEYDOWN` | `0x0100` | `0x42F036` | if `_DAT_007577E8 == 0` enqueue `wParam` with `flags=1` into the 16-entry ring |
| `WM_KEYUP` | `0x0101` | `0x42F072` | enqueue `0x11` (`VK_CONTROL`) / `0x10` (`VK_SHIFT`) only, `flags=0` |
| `WM_MOUSEMOVE` | `0x0200` | `0x42F14D` | `dx = lParam.x - _DAT_007577BC`, `dy = lParam.y - _DAT_007577C0` |
| `WM_LBUTTONDOWN` | `0x0201` | `0x42F0F1` | `_DAT_007577AC |= 1`, `_DAT_007577B0 |= 1`, recompute delta |
| `WM_LBUTTONUP` | `0x0202` | `0x42F134` | clear bit 0 in both |
| `WM_RBUTTONDOWN` | `0x0204` | `0x42F1BD` | `_DAT_007577AC |= 2`, `_DAT_007577B0 |= 2`, recompute delta |
| `WM_RBUTTONUP` | `0x0205` | `0x42F200` | clear bit 1 in both |
| `WM_USER` | `0x0400` | `0x42F22E` | if `_DAT_007786B4` → `FUN_0042D094()` (frame) then `FUN_0042E19C()` (present) |
| `MM_MCINOTIFY` | `0x03B9` | `0x42EFBB` | if `wParam == 1` (`MCI_NOTIFY_SUCCESSFUL`) → `FUN_0042CF80()` (music resume) |
| default | — | `0x42F246` | `DefWindowProcA(hwnd,msg,wParam,lParam)` |

The message set is complete: there is no `WM_USER+0x0D`, no `WM_TIMER`, and no
movie notification handler.

### 5.3 Input ring

The keyboard ring is the same shape as the single-player build:

- values `[16] @ 0x6EAE64`, flags `[16] @ 0x6EB0D0`;
- head `0x6EB798`, tail `0x6EB794` (both masked with `0x0F`);
- `flags = 1` for key-down, `0` for key-up (only Shift/Ctrl are enqueued on
  key-up).

One event is consumed per frame by `FUN_0042855E` (see
`CLIENT_EXE_STATE_MACHINE.md`).

## 6. Thread model

- The only `CreateThread` code reference is the import thunk at `0x439662`
  (`KERNEL32.DLL::CreateThread`), referenced from `0x438B1B` inside the
  CRT/library region. Run-time procedural graph shows no game function
  creating a thread.
- The client therefore runs a single game/UI thread. DirectPlay may create
  internal OS threads inside `DPLAYX.dll`; those are outside the shipped image
  and are not observed here.
- There is no PlayAVI helper thread (the import and all movie code are absent).

## 7. Shutdown

- Menu “Выход” calls the window close path; `FUN_0043BF5C`-equivalent sets
  `_DAT_006EB780`.
- The next WndProc call clears `_DAT_007786B5` and calls `DestroyWindow`.
- `WM_DESTROY` calls `FUN_0042D788`, `FUN_0042D69C`, `FUN_0042D174` and
  `PostQuitMessage(0)`, ending the loop.
- Local unattended run: a clean menu exit produced `CLIENT.LOG` containing
  `" rc=0"` and process exit code 0 (see §8).

## 8. Runtime corroboration (local, not REA-captured)

REA `capture_process_scenario` is unavailable on this Windows host (PTY
descendant-cleanup unsupported), so the run below was performed with the
unmodified binary in an isolated copy and observed without a debugger.

| run | configuration | result |
|---|---|---|
| 1 | only the statically-visible archives (`GRAPH/GAME/MAPS/LIGHTS/OBJECTS/SOUNDS_/QUESTR/FLAMES`) | process exits 1; `CLIENT.LOG` = `ME640R.RES not found rc=16` |
| 2 | add `ME640R.RES` + `ME800R.RES` | process stays in the 60 ms message loop; 640×480 main menu is displayed with **Игра / Новый герой / Установки / Выход** |
| 3 | user clicked “Выход” | clean exit, `CLIENT.LOG` = `" rc=0"`, exit code 0 |

This establishes two facts that are invisible in the defined-string inventory:

1. The client builds four archive names at run time:
   `PLAY<width>R.RES`, `ME<width>R.RES`, `NEW<width>R.RES`,
   `INTO<width>R.RES` (`width` = 640 or 800), from the pieces
   `"PLAY" / "ME" / "NEW" / "INTO"`, `"R"`, `".RES"` at `0x4402d0–0x44032b`.
2. `FUN_0041FF3F` is the loader for `ME<width>R.RES`; it is called from the
   initial transient state 9 before the main menu. On failure it logs
   `"<name> not found rc=16"` through `FUN_004169DA` and returns 0.

## 9. Global anchors used by this document

| item | address |
|---|---|
| PE entry / CRT stub | `0x433A72` |
| CRT startup | `0x435910` |
| WinMain pointer slot | `0x7786D4` |
| WinMain | `0x42EBC8` |
| WndProc | `0x42EF70` |
| message range tree | `0x42F265` |
| frame message handler | `0x42F22E` → `FUN_0042D094` (`0x42D094`) |
| frame loop | `0x42EE??–0x42EF6F` |
| frame budget | `_DAT_006EB778 = 0x3C` |
| quit flag | `_DAT_006EB780` |
| app active / running | `_DAT_007786B4` / `_DAT_007786B5` |
| window handle | `0x77861C` |
| player name buffer | `0x77857C` |
| DirectPlay app GUID copy | `0x7785FC` (source `0x440B7C`, 16 B) |
| data-root prefix | `0x6EAF40` (empty) |
| CD-root probe buffer | `0x6EAEB0` |
| screen state | `_DAT_006EB774` |
| network receive buffer | `0x6EB154` (0x200 B) |
| input ring head/tail | `0x6EB798` / `0x6EB794` |
| input values/flags | `0x6EAE64` / `0x6EB0D0` |
| mouse state / edge | `0x7577AC` / `0x7577B0` |
| mouse delta / cursor | `0x7577B4` / `0x7577B8`; `0x7577BC` / `0x7577C0` |
| CreateThread thunk | `0x439662` (referenced only from CRT `0x438B1B`) |

## 10. Residual unknowns

1. `FUN_004306ED` is identified only as a CRT startup helper.
2. `_DAT_006EB770` (set from `local_88` in `FUN_0042E19C`) is the DirectDraw
   surface/present context; its exact type is not proven beyond the vtable
   calls.
3. The local UI-driving attempt to capture the New Hero and Settings screens
   did not reach them (window focus/hit-test interaction); the labels in §8 are
   observed from the main-menu screenshot only.
