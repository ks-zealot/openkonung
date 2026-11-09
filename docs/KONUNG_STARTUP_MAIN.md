# Konung — Startup, WinMain, message loop and thread model

Target: `konung.res` (PE32/i386, image base `0x400000`,
`sha256 d698c4f231489a372cc25212ab2819720a36d7ce5307a045db684135cf679810`).
All addresses are virtual addresses in `konung.res`.

This document covers the process entry chain, window/message setup, the 60 ms frame
tick, configuration load, shutdown, and the complete threading picture. The
screen state machine built on top of this loop is in `KONUNG_STATE_MACHINE.md`.

## 1. Entry chain (verified)

| step | address | evidence | meaning |
|---|---|---|---|
| PE entry | `0x43F4B4` | `ev_3bcbc2636ff56e085f13366c8e789a2eca8656f6d4db90ce098a9ef47dd1b50d` | 15-byte CRT stub |
| stub body | `0x43F4B4` | same | `MOV [0x00831C84], 0x42B1A8; JMP 0x0044016C` |
| CRT startup | `0x44016C` | `ev_36b475c5f00718ef311ce7401d5eee07871b845f9500a511985ca60e9e9ccd47` | calls the function pointer stored at `0x831C84` |
| WinMain | `0x42B1A8` | `ev_67ecd73a96275c4bda98956fcae9b71f9713f3343f8ed857851b2947b66a710b` | not a Ghidra function; read as raw instructions |

`0x42B1A8` is **WinMain**. Ghidra does not define a function boundary there (the
same is true of the WndProc), so the body was decoded from the artifact bytes
(Capstone, x86-32) and cross-checked with every called function's REA dossier.

## 2. WinMain (`0x42B1A8`, stdcall, `ret 0x10`)

Reconstructed order of operations:

1. `_DAT_0081D5AC = -1` — invalidate the screen state before init.
2. Build `WNDCLASS` on the stack:
   - `style = 3` (`CS_HREDRAW | CS_VREDRAW`);
   - `lpfnWndProc = 0x42B65C`;
   - `cbClsExtra = cbWndExtra = 0`;
   - `hInstance = WinMain.hInstance`;
   - `hIcon = LoadIconA(hInstance, "GAME_ICON" @ 0x4502C2)`;
   - `hCursor = LoadCursorA(NULL, 0x7F00)` (`IDC_ARROW`);
   - `hbrBackground = GetStockObject(4)` (`BLACK_BRUSH`);
   - `lpszMenuName = NULL`, `lpszClassName = "Konung" @ 0x4632BC`.
3. `RegisterClassA(&wc)`.
4. `CreateWindowExA(8, "Konung", "Konung", 0x80000000, 0, 0,
   GetSystemMetrics(0), GetSystemMetrics(1), 0, 0, hInstance, 0)`.
   `8 = WS_EX_TOPMOST`, `0x80000000 = WS_POPUP` — a borderless full-desktop popup.
   The handle is stored at `_DAT_0082D230`; `NULL` aborts with return 0.
5. `ShowWindow(hwnd, nCmdShow)`, `UpdateWindow(hwnd)`, `SetFocus(hwnd)`.
6. `FUN_0043BFE4()` — CRT/argv/environment-style startup helper (only calls
   `FUN_0043F7A2` and `FUN_0043EA69`; exact CRT identity not required by the game).
7. `_DAT_0081D5AC = 9` — initial screen is the **intro poster movie**.
8. `_DAT_0082D2C8 = 1` (app active), `_DAT_0081D5B0 = 0` (mouse buttons).
9. Build the **data root** at `0x81CC2C`:
   - start with the literal `"A:\"` @ `0x4502CC`;
   - try drive letters `'A'..'Z'`, calling the imported `GetDriveTypeA`
     (thunk `0x8400F0`); accept the first drive whose type is `5`
     (`DRIVE_CDROM`);
   - if none, the buffer is set to an empty string (`0x81CC2C = 0`);
   - append `"KONUNG\"` @ `0x4502D0`.
   This is the CD-root search. The install used by the remake is a normal
   directory, so the empty-string path means **relative paths from the current
   directory**.
10. `_DAT_0081D5C8 = 0x3C` — frame budget **60 ms**.
11. `FUN_00429570()` → `_DAT_0081D5D8` (time/random seed initialisation).
12. Initialise globals:
    - `_DAT_0081D618 = 0` (quit request flag);
    - `_DAT_0081D5F4 = 0` (movie/message-pump mode);
    - `_DAT_0081D5E0 = 0`;
    - `_DAT_0081D614 = 1`;
    - ring indices `_DAT_0081D66C = _DAT_0081D668 = 0`;
    - `_DAT_0081D764 = 1` (debug selector initial value);
    - `0x82D238/0x82D23C/0x82D240 = 0` (DirectDraw/DirectSound interface slots).
13. Build the config path: `strcpy(0x81CFF8, 0x81CD30)` then
    `strcat(0x81CFF8, "KONUNG.CFG" @ 0x4502D8)`. At this point `0x81CD30` is an
    empty string, so the config path is the relative name `KONUNG.CFG`.
14. Open and parse `KONUNG.CFG` — see `KONUNG_SAVE_LOAD.md`.
15. `FUN_0042781C(video_mode)` — sets the per-resolution tables and initialises
    DirectDraw. Failure destroys the window and returns 0.
16. `FUN_00429C0C(hwnd)` — DirectSound init (three buffers, 22050/19050/25050 Hz).
17. `FUN_004392B4()` — asset loader (GRAPH/INTR/SOUNDS/FLAMES/GAME/QUEST and the
    `KONUNG.SAV` slot-name table). Failure destroys the window and returns 0.
18. `ShowCursor(0)` — hide the OS cursor (the game draws its own).
19. `_DAT_0082D2C9 = 1` (running), `last_tick = timeGetTime()`.
20. Enter the message/frame loop (below).
21. On exit return `1`.

### 2.1 Message/frame loop (`0x42B58C`–`0x42B647`)

```text
while (_DAT_0082D2C9) {
    if (_DAT_0081D5F4 != 0) {                 // movie mode: blocking pump
        if (GetMessageA(&msg,0,0,0)) {
            TranslateMessage(&msg); DispatchMessageA(&msg);
        }
    } else {                                  // normal mode: non-blocking pump
        if (PeekMessageA(&msg,0,0,0,PM_REMOVE)) {
            TranslateMessage(&msg); DispatchMessageA(&msg);
        } else if (_DAT_0082D2C8) {           // app active
            now = timeGetTime();
            if (now - last_tick >= _DAT_0081D5C8) {  // 60 ms
                last_tick = now;
                SendMessageA(hwnd, 0x0400, 0, 0);    // WM_USER: one frame
            }
        }
    }
}
```

Consequences:

- The game is **event-driven**: one `WM_USER 0x400` frame is sent only when the
  app is active and 60 ms have elapsed since the previous frame.
- There is no `Sleep`/`timeSetEvent`/timer thread; the limiter is
  `timeGetTime` in the main loop.
- Mouse/keyboard messages are still pumped between frames; they only mutate the
  input state/ring and are consumed on the next frame.
- `WM_PAINT` also renders a frame (`FUN_0042CBD0(-1)` + `FUN_0042A470`) when the
  screen state is 0.

## 3. WndProc (`0x42B65C`, stdcall, `ret 0x10`)

Ghidra defines no function here (`resolve_containing_procedure(0x42B65C)` →
`not_in_procedure`). The body was decoded as raw instructions. Argument layout:
`hwnd=[ebp+0x14]`, `msg=[ebp+0x18]`, `wParam=[ebp+0x1C]`, `lParam=[ebp+0x20]`.

### 3.1 Quit pre-check

At entry: if `_DAT_0081D618 != 0` (quit requested), clear it, clear
`_DAT_0082D2C9`, `DestroyWindow(hwnd)`, return 0.

### 3.2 Message table (complete range tree at `0x42B92A`)

| message | value | handler @ | effect |
|---|---|---|---|
| `WM_CREATE` | `0x0001` | `0x42B6C4` | return 0 |
| `WM_DESTROY` | `0x0002` | `0x42B6C9` | `FUN_0042987C()` (sound shutdown), `FUN_00429744()` (DirectDraw shutdown), `PostQuitMessage(0)` |
| `WM_PAINT` | `0x000F` | `0x42B847` | `BeginPaint`; if `_DAT_0081D5AC == 0` → `FUN_0042CBD0(-1)` + `FUN_0042A470()`; `EndPaint` |
| `WM_ACTIVATEAPP` | `0x001C` | `0x42B6B7` | `_DAT_0082D2C8 = (byte)wParam` |
| `WM_SETCURSOR` | `0x0020` | `0x42B8DE` | `SetCursor(0)`, return 1 |
| `WM_KEYDOWN` | `0x0100` | `0x42B6F6` | enqueue `wParam` into the 16-entry ring with `flags=1` |
| `WM_KEYUP` | `0x0101` | `0x42B737` | enqueue `0x10`/`0x11` only (Shift/Ctrl), `flags=0` |
| `WM_MOUSEMOVE` | `0x0200` | `0x42B812` | `dx = lParam.x - _DAT_0081D5C0`, `dy = lParam.y - _DAT_0081D5C4` |
| `WM_LBUTTONDOWN` | `0x0201` | `0x42B7B6` | `_DAT_0081D5B0 |= 1`, `_DAT_0081D5B4 |= 1`, recompute delta |
| `WM_LBUTTONUP` | `0x0202` | `0x42B7F9` | clear bit 0 in both |
| `WM_RBUTTONDOWN` | `0x0204` | `0x42B882` | `_DAT_0081D5B0 |= 2`, `_DAT_0081D5B4 |= 2`, recompute delta |
| `WM_RBUTTONUP` | `0x0205` | `0x42B8C5` | clear bit 1 in both |
| `WM_USER` | `0x0400` | `0x42B8F3` | if `_DAT_0082D2C8` → `FUN_00436214()` (one frame) then `FUN_0042A470()` (present/cursor) |
| `WM_USER+0x0D` | `0x040D` | `0x42B6E1` | `FUN_00445D9A(0)` (statically linked AVIFile/FLIC message handling); if nonzero → `FUN_0042B098()` (movie pump) |
| `MM_MCINOTIFY` | `0x03B9` | `0x42B6A7` | if `wParam == 1` (`MCI_NOTIFY_SUCCESSFUL`) → `FUN_00429630()` (music resume via `mciSendCommandA`) |
| default | — | `0x42B90B` | `DefWindowProcA(hwnd,msg,wParam,lParam)` |

The full key/mouse semantics of these handlers are in `KEY_INPUT_SYSTEM.md` and
`MOUSE_INPUT_SYSTEM.md`; this table is the complete message set of the WndProc.

## 4. Thread model

**The game itself starts no threads.**

- The only `CreateThread` reference in the executable is the CRT helper
  `FUN_00444D1F` (`CreateThread` thunk `0x445CC2`,
  `ev_7380d8a430073023e62ce27ba60918b43a9f0f4ae707c382d3b5d3a79fda3b1c`).
  It creates a thread at `LAB_00444C6A` and waits on an event; it is the CRT
  `_beginthread`/`_beginthreadex` support routine
  (`ev_7343543e041534b6530da4bb42cb25074de3041f1dc4c6af7adfe3527e20c80c`).
- `FUN_00444D1F` has **no callers and no data references**
  (`procedure_callers` empty, `xrefs(0x444D1F)` empty). It is linked CRT code,
  never invoked by the game.
- `ExitThread`, `GetCurrentThread`, `GetCurrentThreadId` are likewise only in
  the CRT import/helper region (`0x445CBC`–`0x445D16`).
- The movie path uses the external `PlayAVI.dll` (`_playAVIFile@8`,
  `ev_60a4959cd094a1f57949417189a0c968c80f9c458229305b0875803da823c63d`).
  Follow-up triage of `PlayAVI.dll` (`sha256 0282620e…142c`, 316 416 B, 2296
  procedures) shows it **does create one helper thread**:
  `FUN_004281F0` installs a thread-local `WH_GETMESSAGE` hook
  (`SetWindowsHookExA(3, …)`) and calls `CreateThread` once
  (`0x405EC0` → `CreateThread`; caller `0x4281F0`) with start routine
  `0x428150`. The thread is guarded by module globals (`DAT_0044576C`), so it
  is singleton. Body reconstructed from raw bytes: it stores its thread id,
  then loops `GetMessageA`-style processing for the active video object
  (`[obj+0x48]`), calls the frame processor `0x429FE4`, and waits on an event
  with a **100 ms** timeout (`WaitForSingleObject`, result `0x102` =
  `WAIT_TIMEOUT` repeats). It is a video-frame pump, not a second game loop.
- The frame tick is a Win32 message (`WM_USER 0x400`) produced by
  `timeGetTime` in the main loop, not a timer or worker thread.

So the documented concurrency model is: **one game/UI thread** + **one
PlayAVI.dll helper thread** (created lazily on the first movie frame, with a
thread-local message hook).

## 5. Shutdown

- Menu item “exit” calls `FUN_0043BF5C("",0)` (`0x436CD9`): it sets
  `_DAT_0081D618 = 1`, shuts down graphics/sound interfaces and copies the
  message into `0x81CE68` (`ev_7788706670363b96fab775d9d89cb879acc7a04fa087cf4c5c204d0d3f3ec0d7`).
- The next WndProc call sees `_DAT_0081D618 != 0`, clears the running flag and
  calls `DestroyWindow`.
- `WM_DESTROY` shuts down sound (`FUN_0042987C`), DirectDraw (`FUN_00429744`)
  and posts `WM_QUIT`, ending the loop.

## 6. Address anchors

| item | address |
|---|---|
| PE entry / CRT stub | `0x43F4B4` |
| CRT startup | `0x44016C` |
| WinMain | `0x42B1A8` |
| WndProc | `0x42B65C` |
| message range tree | `0x42B92A` |
| frame message handler | `0x42B8F3` → `FUN_00436214` (`0x436214`) |
| frame loop | `0x42B58C`–`0x42B647` |
| frame budget global | `_DAT_0081D5C8 = 0x3C` |
| quit flag | `_DAT_0081D618` |
| app active / running | `_DAT_0082D2C8` / `_DAT_0082D2C9` |
| window handle | `_DAT_0082D230` |
| CD data root | `0x81CC2C` |
| path prefix used by loaders | `0x81CD30` |
| config path buffer | `0x81CFF8` |
| CRT thread helper | `FUN_00444D1F` |
| PlayAVI import | `PLAYAVI.DLL::playAVIFile` |

## 7. Residual unknowns

1. `FUN_0043BFE4` — identified only as a CRT startup helper; its exact CRT
   function name is not needed by the game and is UNKNOWN.
2. `_DAT_0081D5D8`, `_DAT_0081D5E0`, `_DAT_0081D614` — initialised here, roles
   not fully resolved (`_81D614` is the trade-toggle state per
   `KEY_INPUT_SYSTEM.md`).
3. `0x03B9` is resolved as `MM_MCINOTIFY`: the MCI music device sends it to the
   game window; `wParam == 1` (`MCI_NOTIFY_SUCCESSFUL`) calls
   `FUN_00429630` to resume/play music (`mciSendCommandA`).
