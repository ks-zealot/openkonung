"""Original half of the original-vs-remake screenshot harness.

    python web/tools/oracle_capture.py [--out DIR] [--pid N]

Drives the live original to a deterministic state (loc 4, leader cell 72,34,
only the leader selected), then for every ambient frame in ``FRAMES`` and every
render-toggle config in ``CONFIGS`` sets ``WORLD_FRAME`` / the three config
globals, parks the game cursor on neutral ground, and captures the client area
to ``original-<frame>-<cfg>.png``.

The counterpart is ``web/tools/oracle_frames.mjs`` (same names, same state);
``web/tools/oracle_diff.py`` pairs them up.

The world frame advances once per 60 ms game frame, so the value is written
``LEAD`` frames early and the phase actually captured is printed. The ambient
table's plateaus are hundreds of frames wide, so a few frames of latency do not
change the compared delta.
"""
from __future__ import annotations

import argparse
import ctypes
import struct
import sys
import zlib
from ctypes import wintypes as wt
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from original_cheat import GameProcess, RENDER_TOGGLES, WORLD_FRAME  # noqa: E402

FRAMES = (0, 6000, 12000, 18000)
#: name -> (shadows, day_night, light_effects)
CONFIGS = {
    "all": (1, 1, 1),
    "noshadow": (0, 1, 1),
    "dayonly": (1, 0, 1),
    "nolight": (1, 1, 0),
}
CELL = (72, 34)
POINTER = (780, 20)
LEAD = 5

user32 = ctypes.windll.user32
gdi32 = ctypes.windll.gdi32

user32.GetClientRect.argtypes = [wt.HWND, ctypes.POINTER(wt.RECT)]
user32.ClientToScreen.argtypes = [wt.HWND, ctypes.POINTER(wt.POINT)]
user32.GetDC.restype = wt.HDC
user32.GetDC.argtypes = [wt.HWND]
user32.ReleaseDC.argtypes = [wt.HWND, wt.HDC]
gdi32.CreateCompatibleDC.restype = wt.HDC
gdi32.CreateCompatibleDC.argtypes = [wt.HDC]
gdi32.CreateCompatibleBitmap.restype = wt.HBITMAP
gdi32.CreateCompatibleBitmap.argtypes = [wt.HDC, ctypes.c_int, ctypes.c_int]
gdi32.SelectObject.argtypes = [wt.HDC, wt.HGDIOBJ]
gdi32.BitBlt.argtypes = [
    wt.HDC, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int,
    wt.HDC, ctypes.c_int, ctypes.c_int, wt.DWORD,
]
gdi32.GetDIBits.argtypes = [
    wt.HDC, wt.HBITMAP, wt.UINT, wt.UINT, wt.LPVOID,
    ctypes.c_void_p, wt.UINT,
]
gdi32.DeleteObject.argtypes = [wt.HGDIOBJ]
gdi32.DeleteDC.argtypes = [wt.HDC]

SRCCOPY = 0x00CC0020


class BITMAPINFOHEADER(ctypes.Structure):
    _fields_ = [
        ("biSize", wt.DWORD),
        ("biWidth", ctypes.c_long),
        ("biHeight", ctypes.c_long),
        ("biPlanes", wt.WORD),
        ("biBitCount", wt.WORD),
        ("biCompression", wt.DWORD),
        ("biSizeImage", wt.DWORD),
        ("biXPelsPerMeter", ctypes.c_long),
        ("biYPelsPerMeter", ctypes.c_long),
        ("biClrUsed", wt.DWORD),
        ("biClrImportant", wt.DWORD),
    ]


class BITMAPINFO(ctypes.Structure):
    _fields_ = [("bmiHeader", BITMAPINFOHEADER), ("bmiColors", wt.DWORD * 3)]


def write_png(path: Path, w: int, h: int, rgba: bytes) -> None:
    def chunk(tag: bytes, data: bytes) -> bytes:
        return (struct.pack(">I", len(data)) + tag + data
                + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF))

    stride = w * 4
    raw = b"".join(b"\0" + rgba[y * stride:(y + 1) * stride] for y in range(h))
    png = (b"\x89PNG\r\n\x1a\n"
           + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
           + chunk(b"IDAT", zlib.compress(raw, 6))
           + chunk(b"IEND", b""))
    path.write_bytes(png)


def capture(hwnd: int, path: Path) -> tuple[int, int]:
    """Copy the window client area from the screen and write it as RGBA PNG."""
    rect = wt.RECT()
    if not user32.GetClientRect(hwnd, ctypes.byref(rect)):
        raise RuntimeError("GetClientRect failed")
    w, h = rect.right - rect.left, rect.bottom - rect.top
    if w <= 0 or h <= 0:
        raise RuntimeError(f"client area is {w}x{h}")
    pt = wt.POINT(0, 0)
    user32.ClientToScreen(hwnd, ctypes.byref(pt))
    screen = user32.GetDC(None)
    mem = gdi32.CreateCompatibleDC(screen)
    bmp = gdi32.CreateCompatibleBitmap(screen, w, h)
    old = gdi32.SelectObject(mem, bmp)
    try:
        if not gdi32.BitBlt(mem, 0, 0, w, h, screen, pt.x, pt.y, SRCCOPY):
            raise RuntimeError("BitBlt failed")
        info = BITMAPINFO()
        info.bmiHeader.biSize = ctypes.sizeof(BITMAPINFOHEADER)
        info.bmiHeader.biWidth = w
        info.bmiHeader.biHeight = -h  # top-down
        info.bmiHeader.biPlanes = 1
        info.bmiHeader.biBitCount = 32
        info.bmiHeader.biCompression = 0  # BI_RGB
        buf = ctypes.create_string_buffer(w * h * 4)
        if gdi32.GetDIBits(mem, bmp, 0, h, buf, ctypes.byref(info), 0) != h:
            raise RuntimeError("GetDIBits failed")
    finally:
        gdi32.SelectObject(mem, old)
        gdi32.DeleteObject(bmp)
        gdi32.DeleteDC(mem)
        user32.ReleaseDC(None, screen)

    src = buf.raw
    out = bytearray(len(src))
    out[0::4] = src[2::4]  # B -> R
    out[1::4] = src[1::4]
    out[2::4] = src[0::4]  # R -> B
    out[3::4] = b"\xff" * (len(src) // 4)
    write_png(path, w, h, bytes(out))
    return w, h


def set_frame(proc: GameProcess, frame: int, words: tuple[int, int, int]) -> None:
    user32.SetForegroundWindow(proc.hwnd)
    proc.write(WORLD_FRAME, struct.pack("<I", max(0, frame - LEAD)))
    shadows, day_night, light_effects = words
    for name, value in (
        ("shadows", shadows),
        ("day_night", day_night),
        ("light_effects", light_effects),
    ):
        proc.write(RENDER_TOGGLES[name], struct.pack("<I", value))


def main(argv: list[str] | None = None) -> int:
    try:
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    except Exception:
        pass
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--out", type=Path,
                    default=Path(__file__).resolve().parent / "oracle")
    ap.add_argument("--pid", type=int, default=None)
    args = ap.parse_args(argv)

    proc = GameProcess.attach(args.pid)
    if proc.screen_state != 0:
        raise SystemExit(f"screen state {proc.screen_state}, expected gameplay "
                         "(run `original_cheat.py new-game --hero 0 --class warrior` first)")
    if proc.hwnd is None:
        raise SystemExit("no game window handle")

    # Use the documented field writer rather than a bespoke path, so the order
    # byte and occupancy bookkeeping match `original_cheat.py teleport`.
    from original_cheat import cmd_teleport
    cmd_teleport(proc, argparse.Namespace(entity="leader", x=CELL[0], y=CELL[1],
                                           facing=4, halt=True))
    # The original only shows the inventory strip once a party cell has been
    # activated (`_DAT_0081D600 = owner + 1`, set by the portrait handler
    # `FUN_00424D8C`); a fresh New Game has no strip. Click the leader's own
    # cell so the capture matches the remake's always-visible leader strip.
    proc.click(35, 35)
    proc.pump(0.2)
    proc.activate()
    proc.move_mouse(*POINTER)
    proc.pump(0.2)

    args.out.mkdir(parents=True, exist_ok=True)
    for frame in FRAMES:
        for name, words in CONFIGS.items():
            set_frame(proc, frame, words)
            proc.move_mouse(*POINTER)
            proc.pump(0.35)
            actual = proc.world_frame % 0x5460
            path = args.out / f"original-{frame}-{name}.png"
            w, h = capture(proc.hwnd, path)
            print(f"original-{frame}-{name}.png  {w}x{h}  "
                  f"phase {actual} (requested {frame})  config={words}")
            if abs(actual - frame) > 60:
                print(f"  warning: phase drifted {actual - frame} frames")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
