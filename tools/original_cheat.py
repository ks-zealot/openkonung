#!/usr/bin/env python3
"""Live cheat engine / scenario adapter for the **original** Konung.

Parity work needs the 1999 game as an oracle in arbitrary states: any hero, any
location, any cell, any stats, any items.  This tool reaches those states the
same way a player does wherever possible:

* it attaches to the running ``konung.res`` process (read/write its memory),
* it drives the game's own UI through window messages (``PostMessage``) so
  New Game, the world map and the Load screen run their original code, and
* it patches the documented runtime tables (``Entity``/item/Groups) for the
  edits the UI cannot make (stats, arbitrary cells, arbitrary instances).

Everything here is derived from the recovered layout; the addresses are the
static VAs listed in ``KONUNG_SAVE_LOAD.md`` / ``KONUNG_SOURCE_OF_TRUTH.md``
(image base ``0x400000``, rebased at attach time).  No code is injected.

Examples::

    python tools/original_cheat.py state
    python tools/original_cheat.py new-game --hero 0 --class warrior
    python tools/original_cheat.py dump leader
    python tools/original_cheat.py set leader str=40 str_cur=40 hp_display=100
    python tools/original_cheat.py give leader 2 18 --qty 1          # Нож
    python tools/original_cheat.py give leader 12 30 --qty 100       # arrows
    python tools/original_cheat.py items leader
    python tools/original_cheat.py discover
    python tools/original_cheat.py travel 4                          # loc id
    python tools/original_cheat.py teleport leader 120 40
    python tools/original_cheat.py snapshot scenario.sa0 --slot 0
    python tools/original_cheat.py load-save scenario.sa0 --slot 0 --apply

``catalog`` and ``selftest`` work without a running game (they read
``konung.res`` from the install directory).

Windows only (kernel32/user32 + ``ctypes``); no third-party packages.
"""

from __future__ import annotations

import argparse
import ctypes
import ctypes.wintypes as wt
import json
import struct
import sys
import time
from pathlib import Path

# ---------------------------------------------------------------------------
# Recovered addresses (VAs in konung.res, image base 0x400000)
# ---------------------------------------------------------------------------

IMAGE_BASE = 0x400000

#: Runtime leader pointer (``Entity*`` inside the entity table).
LEADER_PTR = 0x81D56C
#: Party/selection pointer array (10 x ``Entity*``, zero terminated).
SELECTION = 0x81A814
#: Item-instance table, 8192 x 16 bytes.
ITEMS = 0x659B0C
#: Group table, 200 x 0x20 (same layout as ``Game.N +0x20000``).
GROUPS = 0x67C08C
#: WorldObject source table, 500 x 0x40.
WORLD_OBJECTS = 0x77DEB4
#: Ground-marker pool, 200 x 0x40.
MARKERS = 0x785BB4
#: Entity table, 2000 x 0x100.
ENTITIES = 0x78A250
#: Settlement table, 20 x 0x4B5.
SETTLEMENTS = 0x8149F0
#: B7 quest-flag slots, 300 x u32.
QUEST_FLAGS = 0x615068
#: Projectile/effect pool, 100 x 0x20.
PROJECTILES = 0x80B0D0
#: Dialogue runtime: selectable option records (`0x788DB4`, `_81D6D0` entries,
#: stride 0x20) and the NPC statement records (`0x788FB4`); the NPC pointer and
#: the current option count.
DIALOG_OPTIONS = 0x788DB4
DIALOG_STATEMENTS = 0x788FB4
DIALOG_OPTION_COUNT = 0x81D6D0
DIALOG_NPC = 0x81D574
#: "App active" byte (`WM_ACTIVATEAPP`); the frame update is gated on it.
APP_ACTIVE = 0x82D2C8
#: Actor under the cursor (`FUN_0042260C` hit test), 0 = none.
HOVERED_ACTOR = 0x81D578
#: World camera origin: screen = viewport + entity_projection - camera.
CAMERA_X = 0x81D72C
CAMERA_Y = 0x81D748
#: Discovered world-map flag mask (56 bytes).
DISCOVERED = 0x81CF80
#: Current location id (1-based).
LOCATION = 0x81D6C4
#: World/frame tick counter (increments once per 60 ms frame).
WORLD_FRAME = 0x81D640
#: Top-level screen state: 0 gameplay, 1 menu, 2 new hero, 4 load, ...
SCREEN_STATE = 0x81D5AC
#: Interface mode: 0 world, 1 dialogue, 2 quest log, 3 character, 5 map, 7 detail.
IFACE_MODE = 0x81D610
#: Clamped in-game cursor.
CURSOR_X = 0x81D5C0
CURSOR_Y = 0x81D5C4
#: Cursor clamp maxima: `_81D5C0 <= _81D740`, `_81D5C4 <= _81D75C`
#: (`FUN_00428C00`); they carry the client size (639/479 or 799/599).
CURSOR_MAX_X = 0x81D740
CURSOR_MAX_Y = 0x81D75C
#: World viewport rectangle (left/top/right/bottom in client pixels).
VIEW_LEFT = 0x81D738
VIEW_RIGHT = 0x81D744
VIEW_TOP = 0x81D754
VIEW_BOTTOM = 0x81D760
#: Save-name buffer table (10 x 50 CP866 bytes) written into KONUNG.SAV.
SAVE_NAMES = 0x653518
#: World-map run table.
MAP_ROWS_VA = 0x456A1C  # 361 x u32 row offsets
MAP_RUNS_VA = 0x450EBC  # (location, width) byte pairs, width 0 ends a row
#: 56 CP866 location names, stride 0x28.
LOCATION_NAMES_VA = 0x45B394

ENTITY_COUNT = 2000
ENTITY_STRIDE = 0x100
ITEM_COUNT = 8192
ITEM_STRIDE = 16
GROUP_COUNT = 200
GROUP_STRIDE = 0x20
LOCATION_NAME_COUNT = 56
LOCATION_NAME_STRIDE = 0x28
MAP_ROWS = 361
MAP_W = 502
RESERVED_MASK = list(range(41, 55))  # `0x45AE30`: unrevealable slots

#: Runtime copy of the loaded `Game.N +0x29600` transition table: 17-byte records
#: at `0x7891B4`, active count at `0x81D6E4`. `+0x04` is the destination
#: location id; the world-map click scans this table for it (`FUN_0042260C`).
TRANSITIONS = 0x7891B4
TRANSITION_COUNT = 0x81D6E4
TRANSITION_STRIDE = 17
#: World-map travel consumes one class-11 subtype-20 «Шар магического прыжка»
#: from the leader's inventory (`FUN_00433A8C(0x15)` + `FUN_00421AD0`).
TRAVEL_ORB_CLASS = 0x0B
TRAVEL_ORB_SUBTYPE = 20

#: ``KONUNG.SAn`` layout: (offset, size, runtime VA, label).
SAVE_BLOCKS = (
    (0x000018, 0x4B0, QUEST_FLAGS, "quest flags"),
    (0x0004C8, 0xC80, PROJECTILES, "projectiles"),
    (0x001148, 0x20000, ITEMS, "item instances"),
    (0x021148, 0x3200, MARKERS, "ground markers"),
    (0x024348, 0x1900, GROUPS, "groups"),
    (0x025C48, 0x7D00, WORLD_OBJECTS, "world objects"),
    (0x02D948, 0x7D000, ENTITIES, "entities"),
    (0x0AA948, 0x5E24, SETTLEMENTS, "settlements"),
    (0x0B076C, 0x38, DISCOVERED, "discovered locations"),
)
SAVE_SIZE = 0xB07A4
SAVE_NAME_BYTES = 50

#: Item prototype tables: class -> (name VA, icon/weight VA, stride).
ITEM_TABLES = {
    **{c: (0x459B50, 0x459B88, 0x40) for c in (0, 1, 2, 3, 4, 5, 12)},
    **{c: (0x4580B0, 0x45812A, 0x80) for c in (6, 7, 8)},
    9: (0x459450, 0x4594CA, 0x80),
    **{c: (0x46091C, 0x460996, 0x80) for c in (10, 11)},
}
#: Combat-rating table (`u16` per subtype) for classes 0..=5|12.
ARMOR_TABLE_VA = 0x459B7C
ARMOR_STRIDE = 0x40

#: New-Hero classes: the class box region is `Entity+0x04 + 1` (3..6).
CLASS_NAMES = {5: "warrior", 3: "hunter", 2: "merchant", 4: "chief"}
CLASS_ALIASES = {
    "warrior": 5,
    "hunter": 3,
    "merchant": 2,
    "chief": 4,
    "5": 5,
    "3": 3,
    "2": 2,
    "4": 4,
}

#: Exact rects from ``web/src/frontend/layout.rs`` (`konung.res 0x414B4`,
#: `0x466DC`, `0x46241C`); (x, y, w, h) in client pixels.
MAIN_MENU_RECTS = {
    640: {
        "resume": (233, 130, 192, 31),
        "save": (231, 164, 173, 35),
        "load": (212, 200, 178, 40),
        "new": (224, 241, 128, 40),
        "settings": (215, 282, 118, 39),
        "exit": (222, 327, 78, 44),
    },
    800: {
        "resume": (290, 161, 300, 49),
        "save": (288, 205, 216, 45),
        "load": (265, 250, 223, 53),
        "new": (280, 303, 160, 51),
        "settings": (268, 353, 148, 49),
        "exit": (276, 409, 90, 55),
    },
}
NEWH_RECTS = {
    # `NEWH_HIT` tables: hero identities are regions 0..2, the class boxes are
    # regions 3..6 (keyed here by `Entity+0x04`), the bottom controls are
    # regions 20 (Ok) and 21 (Отмена).
    640: {
        "hero": ((44, 94, 57, 96), (137, 94, 58, 96), (229, 94, 58, 96)),
        "class": {
            2: (481, 68, 99, 20),
            3: (362, 68, 106, 20),
            4: (362, 36, 106, 20),
            5: (481, 36, 99, 20),
        },
        "ok": (65, 427, 100, 33),
        "cancel": (175, 427, 100, 33),
    },
    800: {
        "hero": ((44, 108, 76, 124), (155, 108, 81, 124), (265, 108, 81, 124)),
        "class": {
            2: (582, 81, 96, 20),
            3: (464, 81, 105, 20),
            4: (464, 49, 105, 20),
            5: (582, 49, 96, 20),
        },
        "ok": (85, 527, 100, 33),
        "cancel": (195, 527, 100, 33),
    },
}
SAVE_SLOT_RECTS = {
    640: [(140, 69 + i * 31, 370, 31) for i in range(10)],
    800: [(175, 86, 463, 39), (175, 125, 463, 37), (175, 164, 463, 36),
          (175, 203, 463, 35), (175, 241, 463, 36), (175, 280, 463, 35),
          (175, 319, 463, 35), (175, 357, 463, 35), (175, 396, 463, 34),
          (175, 435, 463, 34)],
}

#: `Entity+0xD4` name ids of the three playable heroes (`hero.rs`).
HERO_NAMES = {124: "Драгомир", 151: "Михаил", 10: "Волк"}

#: Entity fields ``name -> (offset, struct format)``.  Layout: ``parse.rs``
#: `parse_entity_record` (same in ``Game.N`` and the runtime table).
ENTITY_FIELDS = {
    "kind": (0x00, "B"),
    "facing": (0x01, "B"),
    "order": (0x02, "B"),
    "flags": (0x03, "B"),
    "archetype": (0x04, "B"),
    "group": (0x05, "B"),
    "target": (0x06, "B"),
    "x": (0x08, "H"),
    "y": (0x0A, "H"),
    "dest_x": (0x0C, "H"),
    "dest_y": (0x0E, "H"),
    "raw_0x11": (0x11, "B"),
    "level": (0x33, "B"),
    "money": (0x34, "i"),
    "xp": (0x38, "I"),
    "invested_xp": (0x3C, "I"),
    "melee": (0x40, "H"),
    "trade": (0x42, "H"),
    "archery": (0x44, "H"),
    "building": (0x46, "H"),
    "healing": (0x48, "H"),
    "identify": (0x4A, "H"),
    "smithing": (0x4C, "H"),
    "armor": (0x4E, "H"),
    "poison": (0x50, "H"),
    "raw_0x52": (0x52, "H"),
    "dex": (0x54, "H"),
    "dex_cur": (0x56, "H"),
    "cha": (0x58, "H"),
    "cha_cur": (0x5A, "H"),
    "hp": (0x5C, "H"),
    "str": (0x5E, "H"),
    "str_cur": (0x60, "H"),
    "end": (0x62, "H"),
    "end_cur": (0x64, "H"),
    "saved_dex": (0xBC, "H"),
    "saved_cha": (0xBE, "H"),
    "saved_str": (0xC0, "H"),
    "saved_end": (0xC2, "H"),
    "effect_id": (0xCA, "H"),
    "effect_timer": (0xCE, "H"),
    "weapon_slot": (0x12, "B"),
    "ammo": (0xCC, "H"),
    "dialogue": (0xD0, "B"),
    "state_d2": (0xD2, "B"),
    "state_d3": (0xD3, "B"),
    "name_id": (0xD4, "B"),
    "nickname_id": (0xD5, "B"),
    "visual": (0xD6, "B"),
    "palette": (0xD8, "I"),
}
#: Friendly aliases for ``set``.
ENTITY_ALIASES = {
    "hp_display": "hp",  # converted below
    "experience": "xp",
    "skill_melee": "melee",
    "skill_archery": "archery",
    "skill_trade": "trade",
    "skill_building": "building",
    "skill_healing": "healing",
    "skill_identification": "identify",
    "skill_smithing": "smithing",
    "strength": "str",
    "dexterity": "dex",
    "charisma": "cha",
    "endurance": "end",
}

#: Equipment/ammo/additional container sizes.
MAIN_SLOTS = 6
INVENTORY_SLOTS = 32
ADDITIONAL_SLOTS = 5
EQUIP_OFF = 0x66
INVENTORY_OFF = 0x72
ADDITIONAL_OFF = 0xB2
SPECIAL_OFF = 0xCC


class CheatError(RuntimeError):
    """A command could not complete; the CLI prints this without a traceback."""


# ---------------------------------------------------------------------------
# Win32 primitives
# ---------------------------------------------------------------------------

k32 = ctypes.WinDLL("kernel32", use_last_error=True)
user32 = ctypes.WinDLL("user32", use_last_error=True)

PROCESS_VM_READ = 0x0010
PROCESS_VM_WRITE = 0x0020
PROCESS_VM_OPERATION = 0x0008
PROCESS_QUERY_INFORMATION = 0x0400
PROCESS_ALL_ACCESS = 0x1F0FFF

TH32CS_SNAPPROCESS = 0x0002
TH32CS_SNAPMODULE = 0x0008
TH32CS_SNAPMODULE32 = 0x0010

WM_NULL = 0x0000
WM_KEYDOWN = 0x0100
WM_MOUSEMOVE = 0x0200
WM_LBUTTONDOWN = 0x0201
WM_LBUTTONUP = 0x0202
WM_ACTIVATEAPP = 0x001C


class PROCESSENTRY32(ctypes.Structure):
    _fields_ = [
        ("dwSize", wt.DWORD),
        ("cntUsage", wt.DWORD),
        ("th32ProcessID", wt.DWORD),
        ("th32DefaultHeapID", ctypes.POINTER(ctypes.c_ulong)),
        ("th32ModuleID", wt.DWORD),
        ("cntThreads", wt.DWORD),
        ("th32ParentProcessID", wt.DWORD),
        ("pcPriClassBase", ctypes.c_long),
        ("dwFlags", wt.DWORD),
        ("szExeFile", ctypes.c_char * 260),
    ]


class MODULEENTRY32(ctypes.Structure):
    _fields_ = [
        ("dwSize", wt.DWORD),
        ("th32ModuleID", wt.DWORD),
        ("th32ProcessID", wt.DWORD),
        ("GlblcntUsage", wt.DWORD),
        ("ProccntUsage", wt.DWORD),
        ("modBaseAddr", ctypes.POINTER(ctypes.c_byte)),
        ("modBaseSize", wt.DWORD),
        ("hModule", wt.HMODULE),
        ("szModule", ctypes.c_char * 256),
        ("szExePath", ctypes.c_char * 260),
    ]


k32.OpenProcess.restype = wt.HANDLE
k32.OpenProcess.argtypes = [wt.DWORD, wt.BOOL, wt.DWORD]
k32.CloseHandle.argtypes = [wt.HANDLE]
k32.ReadProcessMemory.argtypes = [
    wt.HANDLE, wt.LPCVOID, wt.LPVOID, ctypes.c_size_t, ctypes.POINTER(ctypes.c_size_t)
]
k32.ReadProcessMemory.restype = wt.BOOL
k32.WriteProcessMemory.argtypes = [
    wt.HANDLE, wt.LPVOID, wt.LPCVOID, ctypes.c_size_t, ctypes.POINTER(ctypes.c_size_t)
]
k32.WriteProcessMemory.restype = wt.BOOL
k32.CreateToolhelp32Snapshot.restype = wt.HANDLE
k32.CreateToolhelp32Snapshot.argtypes = [wt.DWORD, wt.DWORD]
k32.Process32First.argtypes = [wt.HANDLE, ctypes.POINTER(PROCESSENTRY32)]
k32.Process32First.restype = wt.BOOL
k32.Process32Next.argtypes = [wt.HANDLE, ctypes.POINTER(PROCESSENTRY32)]
k32.Process32Next.restype = wt.BOOL
k32.Module32First.argtypes = [wt.HANDLE, ctypes.POINTER(MODULEENTRY32)]
k32.Module32First.restype = wt.BOOL
k32.Module32Next.argtypes = [wt.HANDLE, ctypes.POINTER(MODULEENTRY32)]
k32.Module32Next.restype = wt.BOOL
user32.PostMessageA.argtypes = [wt.HWND, wt.UINT, wt.WPARAM, wt.LPARAM]
user32.PostMessageA.restype = wt.BOOL
user32.GetWindowThreadProcessId.argtypes = [wt.HWND, ctypes.POINTER(wt.DWORD)]
user32.IsWindowVisible.argtypes = [wt.HWND]
user32.SetForegroundWindow.argtypes = [wt.HWND]
user32.GetClassNameA.argtypes = [wt.HWND, wt.LPSTR, ctypes.c_int]

ENUM_WINDOW_PROC = ctypes.WINFUNCTYPE(wt.BOOL, wt.HWND, wt.LPARAM)


def list_processes(name: str) -> list[int]:
    """PIDs whose executable name matches ``name`` (case-insensitive)."""
    wanted = name.casefold()
    snap = k32.CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
    if snap in (None, wt.HANDLE(-1).value) or snap == 0xFFFFFFFFFFFFFFFF:
        raise CheatError(f"CreateToolhelp32Snapshot failed ({ctypes.get_last_error()})")
    entry = PROCESSENTRY32()
    entry.dwSize = ctypes.sizeof(PROCESSENTRY32)
    pids: list[int] = []
    try:
        ok = k32.Process32First(snap, ctypes.byref(entry))
        while ok:
            exe = entry.szExeFile.decode("mbcs", "replace")
            if exe.casefold() == wanted:
                pids.append(int(entry.th32ProcessID))
            ok = k32.Process32Next(snap, ctypes.byref(entry))
    finally:
        k32.CloseHandle(snap)
    return pids


def list_modules(pid: int) -> list[tuple[str, int, int]]:
    """``(name, base, size)`` for every module of ``pid``."""
    snap = k32.CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid)
    if not snap or snap == 0xFFFFFFFFFFFFFFFF:
        raise CheatError(f"module snapshot failed ({ctypes.get_last_error()})")
    entry = MODULEENTRY32()
    entry.dwSize = ctypes.sizeof(MODULEENTRY32)
    out: list[tuple[str, int, int]] = []
    try:
        ok = k32.Module32First(snap, ctypes.byref(entry))
        while ok:
            base = ctypes.cast(entry.modBaseAddr, ctypes.c_void_p).value or 0
            out.append((entry.szModule.decode("mbcs", "replace"), base, int(entry.modBaseSize)))
            ok = k32.Module32Next(snap, ctypes.byref(entry))
    finally:
        k32.CloseHandle(snap)
    return out


# ---------------------------------------------------------------------------
# Process wrapper
# ---------------------------------------------------------------------------


class GameProcess:
    """Attached ``konung.res`` process: VA-translated reads/writes + UI input."""

    def __init__(self, pid: int, handle: int, base: int, hwnd: int | None):
        self.pid = pid
        self.handle = handle
        self.base = base
        self.delta = base - IMAGE_BASE
        self.hwnd = hwnd

    # -- lifecycle ---------------------------------------------------------
    @classmethod
    def attach(cls, pid: int | None = None, exe: str = "konung.res") -> "GameProcess":
        if pid is None:
            pids = list_processes(exe)
            if not pids:
                raise CheatError(
                    f"no running '{exe}' process; start the original game first "
                    f"(or pass --pid)"
                )
            if len(pids) > 1:
                raise CheatError(
                    f"several '{exe}' processes: {pids}; pass --pid"
                )
            pid = pids[0]
        handle = k32.OpenProcess(PROCESS_ALL_ACCESS, False, pid)
        if not handle:
            raise CheatError(
                f"OpenProcess({pid}) failed ({ctypes.get_last_error()}); "
                f"run as administrator?"
            )
        modules = list_modules(pid)
        bases = [base for name, base, _ in modules if name.casefold() == exe.casefold()]
        if not bases:
            raise CheatError(f"module '{exe}' not found in pid {pid}")
        base = bases[0]
        hwnd = find_window(pid)
        return cls(pid, handle, base, hwnd)

    def close(self) -> None:
        if self.handle:
            k32.CloseHandle(self.handle)
            self.handle = 0

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        self.close()

    @property
    def install_dir(self) -> Path | None:
        try:
            import ctypes.wintypes as _w  # noqa: F401

            buf = ctypes.create_unicode_buffer(1024)
            size = wt.DWORD(len(buf))
            k32.QueryFullProcessImageNameW.argtypes = [
                wt.HANDLE, wt.DWORD, wt.LPWSTR, ctypes.POINTER(wt.DWORD)
            ]
            if k32.QueryFullProcessImageNameW(self.handle, 0, buf, ctypes.byref(size)):
                return Path(buf.value).parent
        except Exception:
            pass
        return None

    # -- memory ------------------------------------------------------------
    def _addr(self, va: int) -> int:
        return va + self.delta

    def read(self, va: int, size: int) -> bytes:
        buf = (ctypes.c_char * size)()
        got = ctypes.c_size_t(0)
        if not k32.ReadProcessMemory(
            self.handle, ctypes.c_void_p(self._addr(va)), buf, size, ctypes.byref(got)
        ) or got.value != size:
            raise CheatError(
                f"ReadProcessMemory(0x{va:08X}, {size}) failed "
                f"({ctypes.get_last_error()}, read {got.value})"
            )
        return buf.raw

    def write(self, va: int, data: bytes) -> None:
        buf = ctypes.create_string_buffer(data, len(data))
        put = ctypes.c_size_t(0)
        if not k32.WriteProcessMemory(
            self.handle, ctypes.c_void_p(self._addr(va)), buf, len(data), ctypes.byref(put)
        ) or put.value != len(data):
            raise CheatError(
                f"WriteProcessMemory(0x{va:08X}, {len(data)}) failed "
                f"({ctypes.get_last_error()}, wrote {put.value})"
            )

    def u8(self, va: int) -> int:
        return self.read(va, 1)[0]

    def i8(self, va: int) -> int:
        return struct.unpack("<b", self.read(va, 1))[0]

    def u16(self, va: int) -> int:
        return struct.unpack("<H", self.read(va, 2))[0]

    def u32(self, va: int) -> int:
        return struct.unpack("<I", self.read(va, 4))[0]

    def i32(self, va: int) -> int:
        return struct.unpack("<i", self.read(va, 4))[0]

    def put_u8(self, va: int, value: int) -> None:
        self.write(va, struct.pack("<B", value & 0xFF))

    def put_u16(self, va: int, value: int) -> None:
        self.write(va, struct.pack("<H", value & 0xFFFF))

    def put_u32(self, va: int, value: int) -> None:
        self.write(va, struct.pack("<I", value & 0xFFFFFFFF))

    # -- live state --------------------------------------------------------
    @property
    def screen_state(self) -> int:
        return self.i32(SCREEN_STATE)

    @property
    def iface_mode(self) -> int:
        return self.i32(IFACE_MODE)

    @property
    def location(self) -> int:
        return self.u32(LOCATION)

    @property
    def world_frame(self) -> int:
        return self.u32(WORLD_FRAME)

    @property
    def video_mode(self) -> int:
        """The `KONUNG.CFG` video word as currently effective (0x111 / 0x114).

        The original does not keep the raw word in a stable global after
        `FUN_0042781C`; the client size is recovered from the cursor clamp
        maxima instead and reported in the same encoding for convenience.
        """
        return 0x111 if self.logical_size[0] <= 640 else 0x114

    @property
    def logical_size(self) -> tuple[int, int]:
        try:
            width = self.i32(CURSOR_MAX_X) + 1
            height = self.i32(CURSOR_MAX_Y) + 1
            if width in (640, 800) and height in (480, 600):
                return (width, height)
        except CheatError:
            pass
        return (640, 480)

    def in_game(self) -> bool:
        return self.screen_state == 0

    def leader_ptr(self) -> int:
        return self.u32(LEADER_PTR)

    def leader_index(self) -> int | None:
        if not self.in_game():
            return None
        ptr = self.leader_ptr()
        if ENTITIES <= ptr < ENTITIES + ENTITY_COUNT * ENTITY_STRIDE:
            return (ptr - ENTITIES) // ENTITY_STRIDE
        return None

    def selection(self) -> list[int]:
        raw = self.read(SELECTION, 10 * 4)
        return [p for (p,) in struct.iter_unpack("<I", raw) if p]

    def group(self, index: int) -> dict:
        raw = self.read(GROUPS + index * GROUP_STRIDE, GROUP_STRIDE)
        first, growth = struct.unpack_from("<Hi", raw, 0)
        return {
            "index": index,
            "first_entity": first,
            "count": raw[0x1C],
            "loc_id": raw[0x1B],
            "class": raw[0x1E],
            "activity": raw[0x1D],
            "visited": raw[0x18],
            "capacity": raw[0x19],
            "target": raw[0x1A],
        }

    def groups(self) -> list[dict]:
        raw = self.read(GROUPS, GROUP_COUNT * GROUP_STRIDE)
        out = []
        for i in range(GROUP_COUNT):
            g = raw[i * GROUP_STRIDE:(i + 1) * GROUP_STRIDE]
            out.append({
                "index": i,
                "first_entity": struct.unpack_from("<H", g, 0)[0],
                "count": g[0x1C],
                "loc_id": g[0x1B],
                "class": g[0x1E],
                "activity": g[0x1D],
                "visited": g[0x18],
                "capacity": g[0x19],
                "target": g[0x1A],
            })
        return out

    # -- input -------------------------------------------------------------
    def _post(self, msg: int, wparam: int, lparam: int) -> None:
        if not self.hwnd:
            raise CheatError("game window not found")
        user32.PostMessageA(self.hwnd, msg, wparam, lparam)

    def screen_to_client(self, x: int, y: int) -> tuple[int, int]:
        # The game's logical framebuffer fills the whole client area (the
        # client is created at exactly 640x480 / 800x600).
        return x, y

    def move_mouse(self, x: int, y: int) -> None:
        """Move the game's integrated cursor to client ``(x, y)``.

        ``WM_MOUSEMOVE`` only stores ``lParam - _81D5C0`` (a delta); the
        per-frame integrator adds it, so one message lands the cursor exactly on
        the target even though the OS pointer never moves.
        """
        cx, cy = self.screen_to_client(x, y)
        self._post(WM_MOUSEMOVE, 0, ((cy & 0xFFFF) << 16) | (cx & 0xFFFF))

    def click(self, x: int, y: int, hold: float = 0.14,
              right: bool = False) -> None:
        """Synthesise a click at client ``(x, y)`` (press held one frame)."""
        self.move_mouse(x, y)
        time.sleep(0.10)
        self.move_mouse(x, y)
        lparam = ((y & 0xFFFF) << 16) | (x & 0xFFFF)
        down, up = (0x0204, 0x0205) if right else (0x0201, 0x0202)
        self._post(down, 2 if right else 1, lparam)
        time.sleep(hold)
        self._post(up, 0, lparam)
        time.sleep(0.12)

    def key(self, vk: int, settle: float = 0.12) -> None:
        self._post(WM_KEYDOWN, vk, 0x00010001)
        time.sleep(settle)

    def activate(self) -> None:
        """Keep the original's frame loop running.

        The game advances `FUN_00436214` only while `_DAT_0082D2C8` (the
        `WM_ACTIVATEAPP` flag) is set; a shell-driven session loses that when
        the console takes focus. Restore the window and write the documented
        byte directly - a *posted* activation message can be dropped while the
        loop is blocked in `GetMessageA`.
        """
        if self.hwnd:
            user32.ShowWindow(self.hwnd, 9)  # SW_RESTORE
            user32.SetForegroundWindow(self.hwnd)
        self._post(WM_ACTIVATEAPP, 1, 0)
        try:
            self.put_u8(APP_ACTIVE, 1)
        except CheatError:
            pass

    def pump(self, seconds: float) -> None:
        """Keep the message loop awake for ``seconds`` (posted WM_NULL)."""
        end = time.monotonic() + seconds
        while time.monotonic() < end:
            self._post(WM_NULL, 0, 0)
            time.sleep(0.03)
        self._post(WM_NULL, 0, 0)

    def wait_for(self, predicate, timeout: float, what: str = "condition",
                 pump: bool = True) -> None:
        end = time.monotonic() + timeout
        self.activate()
        while time.monotonic() < end:
            try:
                if predicate():
                    return
            except CheatError:
                pass
            if pump:
                self.pump(0.08)
            else:
                time.sleep(0.05)
        raise CheatError(f"timeout after {timeout:.0f}s waiting for {what}")

    # -- entities ----------------------------------------------------------
    def entity_ptr(self, index: int) -> int:
        if not 0 <= index < ENTITY_COUNT:
            raise CheatError(f"entity index {index} outside 0..{ENTITY_COUNT - 1}")
        return ENTITIES + index * ENTITY_STRIDE

    def entity_bytes(self, index: int) -> bytes:
        return self.read(self.entity_ptr(index), ENTITY_STRIDE)

    def entity_field(self, index: int, name: str) -> int:
        off, fmt = ENTITY_FIELDS[ENTITY_ALIASES.get(name, name)]
        size = struct.calcsize(fmt)
        raw = self.read(self.entity_ptr(index) + off, size)
        return struct.unpack("<" + fmt, raw)[0]

    def set_entity_field(self, index: int, name: str, value: int) -> None:
        if name in ENTITY_ALIASES:
            name = ENTITY_ALIASES[name]
        off, fmt = ENTITY_FIELDS[name]
        ptr = self.entity_ptr(index) + off
        size = struct.calcsize(fmt)
        if fmt == "B" and not 0 <= value <= 0xFF:
            raise CheatError(f"{name}: {value} does not fit u8")
        if fmt == "H" and not 0 <= value <= 0xFFFF:
            raise CheatError(f"{name}: {value} does not fit u16")
        if fmt == "I":
            value &= 0xFFFFFFFF
        if fmt == "i":
            value = struct.unpack("<i", struct.pack("<I", value & 0xFFFFFFFF))[0]
        self.write(ptr, struct.pack("<" + fmt, value))

    def resolve_entity(self, spec: str) -> int:
        """``leader`` / ``selN`` / ``partyN`` / numeric index -> entity index."""
        s = str(spec).strip().lower()
        if s in ("leader", "main", "me", "self"):
            idx = self.leader_index()
            if idx is None:
                raise CheatError("no live leader (start a game first)")
            return idx
        if s in ("sel", "selected"):
            sel = self.selection()
            if not sel:
                raise CheatError("nothing selected")
            return (sel[0] - ENTITIES) // ENTITY_STRIDE
        if s.startswith("party"):
            n = int(s[5:] or 0)
            sel = self.selection()
            if n >= len(sel):
                raise CheatError(f"party{ n } not in the selection list ({len(sel)})")
            return (sel[n] - ENTITIES) // ENTITY_STRIDE
        if s.startswith("sel"):
            n = int(s[3:] or 0)
            sel = self.selection()
            if n >= len(sel):
                raise CheatError(f"sel{n} not in the selection list ({len(sel)})")
            return (sel[n] - ENTITIES) // ENTITY_STRIDE
        try:
            return int(s, 0)
        except ValueError:
            raise CheatError(f"unknown entity selector '{spec}'") from None

    def party_indices(self) -> list[int]:
        """Members of group 0 (the reserved party group), group order wins."""
        g = self.group(0)
        start = g["first_entity"]
        return [i for i in range(start, start + g["count"]) if 0 <= i < ENTITY_COUNT]


def find_window(pid: int) -> int | None:
    """Top-level visible window of ``pid`` (the game's `Konung` class window)."""
    found: list[tuple[int, str]] = []

    def callback(hwnd, _):
        owner = wt.DWORD(0)
        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(owner))
        if owner.value == pid and user32.IsWindowVisible(hwnd):
            buf = ctypes.create_string_buffer(256)
            user32.GetClassNameA(hwnd, buf, 256)
            found.append((hwnd, buf.value.decode("mbcs", "replace")))
        return True

    user32.EnumWindows(ENUM_WINDOW_PROC(callback), 0)
    if not found:
        return None
    for hwnd, cls in found:
        if cls == "Konung":
            return hwnd
    return found[0][0]


# ---------------------------------------------------------------------------
# Offline konung.res image (for catalog/selftest)
# ---------------------------------------------------------------------------


class PeImage:
    """VA-addressable view of a PE file (sections at ``IMAGE_BASE``)."""

    def __init__(self, path: Path):
        self.path = Path(path)
        self.data = self.path.read_bytes()
        if self.data[:2] != b"MZ":
            raise CheatError(f"{path} is not a PE image")
        pe = struct.unpack_from("<I", self.data, 0x3C)[0]
        if self.data[pe:pe + 4] != b"PE\0\0":
            raise CheatError(f"{path} has no PE signature")
        nsec = struct.unpack_from("<H", self.data, pe + 6)[0]
        opt = pe + 24
        self.image_base = struct.unpack_from("<I", self.data, opt + 28)[0]
        self.sections = []
        so = opt + struct.unpack_from("<H", self.data, pe + 20)[0]
        for i in range(nsec):
            o = so + i * 40
            name = self.data[o:o + 8].rstrip(b"\0").decode("latin1")
            vsize, vaddr, rawsize, rawptr = struct.unpack_from("<IIII", self.data, o + 8)
            self.sections.append((name, vaddr, max(vsize, rawsize), rawptr, rawsize))

    def read(self, va: int, size: int) -> bytes:
        rva = va - self.image_base
        for name, vaddr, vsize, rawptr, rawsize in self.sections:
            if vaddr <= rva < vaddr + vsize:
                off = rawptr + (rva - vaddr)
                if off + size > len(self.data) or rva + size > vaddr + vsize:
                    raise CheatError(
                        f"read 0x{va:08X}+{size} crosses section {name} end"
                    )
                return self.data[off:off + size]
        raise CheatError(f"0x{va:08X} is not inside any section of {self.path.name}")

    def u8(self, va: int) -> int:
        return self.read(va, 1)[0]

    def u16(self, va: int) -> int:
        return struct.unpack("<H", self.read(va, 2))[0]

    def u32(self, va: int) -> int:
        return struct.unpack("<I", self.read(va, 4))[0]


# ---------------------------------------------------------------------------
# Item templates (names/weights/icons/ratings)
# ---------------------------------------------------------------------------


class ItemCatalog:
    """Item prototype lookups over any ``read(va, size)`` source."""

    def __init__(self, read):
        self._read = read
        self._cache: dict[int, bytes] = {}

    def _table(self, class_id: int) -> bytes:
        name_va, icon_va, stride = ITEM_TABLES[class_id]
        # The shared stride-0x40 table (classes 0..=5 and 12) carries names up to
        # subtype 124; the stride-0x80 tables stop around 18/62/60.
        count = 128
        key = class_id
        if key not in self._cache:
            self._cache[key] = self._read(icon_va, stride * count)
        return self._cache[key]

    def name(self, class_id: int, subtype: int) -> str:
        name_va, _, stride = ITEM_TABLES[class_id]
        base = self._cache.get(("name", class_id))
        if base is None:
            count = 128
            base = self._read(name_va, stride * count)
            self._cache[("name", class_id)] = base
        o = subtype * stride
        if o + stride > len(base):
            return ""
        raw = base[o:o + stride].split(b"\0")[0]
        return raw.decode("cp866", "replace").strip()

    def info(self, class_id: int, subtype: int) -> dict | None:
        if class_id not in ITEM_TABLES:
            return None
        name_va, icon_va, stride = ITEM_TABLES[class_id]
        table = self._table(class_id)
        o = subtype * stride
        if o + stride > len(table):
            return None
        weight = struct.unpack_from("<h", table, o)[0]
        icon = struct.unpack_from("<H", table, o + 2)[0]
        if class_id == 12:
            world = 0x99
        elif class_id == 10:
            world = 0xA3
        else:
            world = struct.unpack_from("<H", table, o + 4)[0]
        name = self.name(class_id, subtype)
        if not name:
            return None
        rating = 0
        if class_id in (0, 1, 2, 3, 4, 5, 12):
            rating = self._read(ARMOR_TABLE_VA + subtype * ARMOR_STRIDE, 2)[0] | (
                self._read(ARMOR_TABLE_VA + subtype * ARMOR_STRIDE + 1, 1)[0] << 8
            )
        price_off = 0x36 if stride == 0x40 else 0x78
        name_raw = self._cache[("name", class_id)]
        price = struct.unpack_from("<h", name_raw, o + price_off)[0] if o + price_off + 2 <= len(name_raw) else 0
        durability = struct.unpack_from("<H", name_raw, o + 0x2E)[0] if stride == 0x40 else 0
        return {
            "class": class_id,
            "subtype": subtype,
            "name": name,
            "icon": icon,
            "weight": weight,
            "rating": rating,
            "price": price,
            "durability": durability,
            "world_sprite": world,
        }

    def entries(self, class_id: int) -> list[dict]:
        stride = ITEM_TABLES[class_id][2]
        limit = 128
        out = []
        empty = 0
        for subtype in range(limit):
            info = self.info(class_id, subtype)
            if info is None:
                empty += 1
                if empty >= 4:
                    break
                continue
            empty = 0
            out.append(info)
        return out


# ---------------------------------------------------------------------------
# Save layout / snapshots
# ---------------------------------------------------------------------------


def build_snapshot(proc: GameProcess) -> bytes:
    """Read the live runtime blocks into an exact ``KONUNG.SAn`` image."""
    buf = bytearray(SAVE_SIZE)
    leader = proc.leader_index()
    if leader is None:
        raise CheatError("no live leader: start a game before taking a snapshot")
    struct.pack_into("<I", buf, 0x00, proc.u32(0x81D5EC))
    struct.pack_into("<I", buf, 0x04, proc.u32(0x81D604))
    struct.pack_into("<I", buf, 0x08, proc.world_frame)
    struct.pack_into("<I", buf, 0x0C, leader)
    struct.pack_into("<I", buf, 0x10, proc.u32(0x81D6A0))
    struct.pack_into("<I", buf, 0x14, proc.u32(0x81D6A8))
    for offset, size, va, label in SAVE_BLOCKS:
        data = proc.read(va, size)
        if len(data) != size:
            raise CheatError(f"short read for {label}")
        buf[offset:offset + size] = data
    return bytes(buf)


def snapshot_summary(data: bytes) -> dict:
    leader_index = struct.unpack_from("<I", data, 0x0C)[0]
    entity_base = 0x02D948 + leader_index * ENTITY_STRIDE
    cell = struct.unpack_from("<HH", data, entity_base + 0x08)
    level = data[entity_base + 0x33]
    money = struct.unpack_from("<i", data, entity_base + 0x34)[0]
    return {
        "size": len(data),
        "leader_index": leader_index,
        "leader_cell": list(cell),
        "leader_level": level,
        "leader_money": money,
        "world_frame": struct.unpack_from("<I", data, 0x08)[0],
        "discovered": sum(1 for b in data[0x0B076C:0x0B076C + 56] if b),
    }


def read_save_names(install: Path) -> bytes:
    path = install / "KONUNG.SAV"
    if path.exists():
        data = path.read_bytes()
        return data[:500].ljust(500, b"\0")
    return b"\0" * 500


def save_slot_name(install: Path, slot: int) -> str:
    raw = read_save_names(install)[slot * SAVE_NAME_BYTES:(slot + 1) * SAVE_NAME_BYTES]
    return raw.split(b"\0")[0].decode("cp866", "replace").strip()


def write_save_name_table(install: Path, slot: int, name: str) -> None:
    path = install / "KONUNG.SAV"
    data = bytearray(read_save_names(install))
    raw = name.encode("cp866", "replace")[:SAVE_NAME_BYTES - 1]
    start = slot * SAVE_NAME_BYTES
    data[start:start + SAVE_NAME_BYTES] = raw.ljust(SAVE_NAME_BYTES, b"\0")
    path.write_bytes(bytes(data[:500]))


# ---------------------------------------------------------------------------
# Display helpers
# ---------------------------------------------------------------------------


def decode_entity(raw: bytes) -> dict:
    out: dict = {}
    for name, (off, fmt) in ENTITY_FIELDS.items():
        out[name] = struct.unpack_from("<" + fmt, raw, off)[0]
    out["equipment"] = list(struct.unpack_from("<6H", raw, EQUIP_OFF))
    out["inventory"] = list(struct.unpack_from("<32H", raw, INVENTORY_OFF))
    out["additional"] = list(struct.unpack_from("<5H", raw, ADDITIONAL_OFF))
    out["special"] = struct.unpack_from("<H", raw, SPECIAL_OFF)[0]
    return out


def location_name(proc: GameProcess, loc_id: int) -> str:
    if not 1 <= loc_id <= LOCATION_NAME_COUNT:
        return f"location {loc_id}"
    raw = proc.read(LOCATION_NAMES_VA + (loc_id - 1) * LOCATION_NAME_STRIDE,
                    LOCATION_NAME_STRIDE)
    try:
        return raw.split(b"\0")[0].decode("cp866", "replace").strip()
    except Exception:
        return f"location {loc_id}"


def instance_table(proc: GameProcess) -> bytes:
    return proc.read(ITEMS, ITEM_COUNT * ITEM_STRIDE)


def decode_instance(table: bytes, item_id: int) -> dict | None:
    if item_id == 0 or item_id >= ITEM_COUNT:
        return None
    o = item_id * ITEM_STRIDE
    rec = table[o:o + ITEM_STRIDE]
    cls = rec[0]
    if cls == 0xFF:
        return None
    return {
        "id": item_id,
        "class": cls,
        "condition": struct.unpack_from("<b", rec, 1)[0],
        "modifier": struct.unpack_from("<b", rec, 2)[0],
        "subtype": rec[3],
        "value": struct.unpack_from("<I", rec, 4)[0],
        "durability": struct.unpack_from("<f", rec, 4)[0],
        "max_durability": struct.unpack_from("<f", rec, 8)[0],
        "quantity": struct.unpack_from("<I", rec, 4)[0],
        # Class 12 stores an integer stack; class 9 stores the charge count as
        # an `f32` in the same word.
        "stack": (struct.unpack_from("<I", rec, 4)[0] if cls == 12 else
                  round(struct.unpack_from("<f", rec, 4)[0]) if cls == 9 else None),
        "poison": struct.unpack_from("<H", rec, 12)[0],
        "flags": struct.unpack_from("<H", rec, 14)[0],
    }


def slot_iids(ent: dict) -> list[tuple[str, int, int]]:
    out = []
    for i, item in enumerate(ent["equipment"]):
        out.append(("equip", i, item))
    for i, item in enumerate(ent["inventory"]):
        out.append(("inv", i, item))
    for i, item in enumerate(ent["additional"]):
        out.append(("add", i, item))
    out.append(("ammo", 0, ent["special"]))
    return out


def format_item(catalog: ItemCatalog | None, inst: dict | None) -> str:
    if inst is None:
        return "-"
    name = ""
    if catalog is not None:
        info = catalog.info(inst["class"], inst["subtype"])
        if info:
            name = " " + info["name"]
    cls, sub = inst["class"], inst["subtype"]
    detail = ""
    if cls == 12:
        detail = f" x{inst['quantity']}"
    elif cls == 9:
        qty = struct.unpack("<f", struct.pack("<I", inst["quantity"]))[0]
        detail = f" x{qty:g}"
    elif cls not in (6, 7, 8, 11) and inst["max_durability"] > 0:
        detail = f" {inst['durability']:.0f}/{inst['max_durability']:.0f}"
    cond = "" if inst["condition"] in (-1, 0xFF) else f" c={inst['condition']}"
    return f"id{inst['id']} c{cls}s{sub}{detail}{cond}{name}"


# ---------------------------------------------------------------------------
# Snapshot diffing (the oracle for "what exactly did this action write?")
# ---------------------------------------------------------------------------

HEADER_FIELDS = (
    ("option_flag_2", 0x00, "I"),
    ("option_flag_3", 0x04, "I"),
    ("world_tick", 0x08, "I"),
    ("leader_index", 0x0C, "I"),
    ("projectile_count", 0x10, "I"),
    ("marker_count", 0x14, "I"),
)

ITEM_DIFF_FIELDS = (
    ("class", 0x00, "B"),
    ("condition", 0x01, "b"),
    ("modifier", 0x02, "b"),
    ("subtype", 0x03, "B"),
    ("word_04", 0x04, "I"),
    ("max_durability", 0x08, "f"),
    ("poison", 0x0C, "H"),
    ("flags", 0x0E, "H"),
)

GROUP_DIFF_FIELDS = (
    ("first_entity", 0x00, "H"),
    ("growth_timer", 0x02, "i"),
    ("countdown", 0x06, "h"),
    ("rect_a_x1", 0x08, "H"),
    ("rect_b_x1", 0x0A, "H"),
    ("rect_a_x2", 0x0C, "H"),
    ("rect_b_x2", 0x0E, "H"),
    ("rect_a_y1", 0x10, "H"),
    ("rect_b_y1", 0x12, "H"),
    ("rect_a_y2", 0x14, "H"),
    ("rect_b_y2", 0x16, "H"),
    ("visited", 0x18, "B"),
    ("capacity", 0x19, "B"),
    ("target_group", 0x1A, "B"),
    ("loc_id", 0x1B, "B"),
    ("entity_count", 0x1C, "B"),
    ("activity", 0x1D, "B"),
    ("class", 0x1E, "B"),
)

WORLD_OBJECT_DIFF_FIELDS = (
    ("activation_00", 0x00, "H"),
    ("activation_04", 0x04, "H"),
    ("cell_x", 0x08, "H"),
    ("cell_y", 0x0A, "H"),
    ("sprite", 0x0C, "H"),
    ("source_index", 0x0E, "H"),
    ("money", 0x10, "i"),
    ("loc_id", 0x12, "H"),
) + tuple((f"item[{i}]", 0x14 + i * 2, "H") for i in range(22))

SETTLEMENT_DIFF_FIELDS = (
    ("flag_00", 0x00, "B"),
    ("flag_01", 0x01, "B"),
    ("group_id", 0x02, "B"),
    ("loc_id", 0x03, "B"),
    ("timer2", 0x04, "H"),
    ("timer4", 0x08, "H"),
    ("timer3", 0x0C, "H"),
    ("progress_14", 0x14, "H"),
    ("flags_4b2", 0x4B2, "B"),
) + tuple(
    (f"waypoint[{w}].{name}", 0x3D0 + w * 8 + off, fmt)
    for w in range(8)
    for name, off, fmt in (("facing", 0, "B"), ("dialogue", 1, "B"),
                           ("owner", 2, "H"), ("x", 4, "H"), ("y", 6, "H"))
) + tuple(
    (f"target[{t}].{name}", 0x18 + t * 8 + off, fmt)
    for t in range(12)
    for name, off, fmt in (("key", 0, "B"), ("state", 1, "B"),
                           ("prop", 2, "H"), ("timer", 6, "H"))
)


def _entity_diff_fields() -> tuple:
    fields = [(name, off, fmt) for name, (off, fmt) in
              sorted(ENTITY_FIELDS.items(), key=lambda kv: kv[1][0])]
    fields += [(f"equipment[{i}]", EQUIP_OFF + i * 2, "H") for i in range(MAIN_SLOTS)]
    fields += [(f"inventory[{i}]", INVENTORY_OFF + i * 2, "H") for i in range(INVENTORY_SLOTS)]
    fields += [(f"additional[{i}]", ADDITIONAL_OFF + i * 2, "H") for i in range(ADDITIONAL_SLOTS)]
    fields.append(("special", SPECIAL_OFF, "H"))
    return tuple(fields)


ENTITY_DIFF_FIELDS = _entity_diff_fields()


def diff_block(a: bytes, b: bytes, base: int, stride: int, count: int,
               fields: tuple, only: list[int] | None = None) -> list[dict]:
    """Per-record field/raw-byte differences between two save images."""
    zeros = b"\0" * stride
    out: list[dict] = []
    indices = only if only is not None else range(count)
    for i in indices:
        if not 0 <= i < count:
            continue
        ra = a[base + i * stride:base + (i + 1) * stride]
        rb = b[base + i * stride:base + (i + 1) * stride]
        if ra == rb:
            continue
        changes: dict[str, tuple] = {}
        covered: set[int] = set()
        for name, off, fmt in fields:
            size = struct.calcsize("<" + fmt)
            covered.update(range(off, off + size))
            va = struct.unpack_from("<" + fmt, ra, off)[0]
            vb = struct.unpack_from("<" + fmt, rb, off)[0]
            if va != vb and not (
                isinstance(va, float) and isinstance(vb, float)
                and va != va and vb != vb
            ):
                changes[name] = (va, vb)
        raw = {f"+0x{off:02X}": (ra[off], rb[off])
               for off in range(stride)
               if off not in covered and ra[off] != rb[off]}
        out.append({
            "index": i,
            "created": ra == zeros,
            "removed": rb == zeros,
            "fields": changes,
            "raw": raw,
        })
    return out


def diff_header(a: bytes, b: bytes) -> dict:
    out = {}
    for name, off, fmt in HEADER_FIELDS:
        va = struct.unpack_from("<" + fmt, a, off)[0]
        vb = struct.unpack_from("<" + fmt, b, off)[0]
        if va != vb:
            out[name] = (va, vb)
    return out


def diff_quest_flags(a: bytes, b: bytes) -> list[dict]:
    out = []
    for i in range(300):
        off = 0x18 + i * 4
        va = struct.unpack_from("<I", a, off)[0]
        vb = struct.unpack_from("<I", b, off)[0]
        if va != vb:
            out.append({"index": i, "before": va, "after": vb})
    return out


def diff_discovered(a: bytes, b: bytes) -> list[dict]:
    out = []
    for i in range(56):
        off = 0x0B076C + i
        if a[off] != b[off]:
            out.append({"index": i, "before": a[off], "after": b[off]})
    return out


DIFF_BLOCKS = {
    "groups": lambda a, b: diff_block(a, b, 0x024348, GROUP_STRIDE, GROUP_COUNT,
                                      GROUP_DIFF_FIELDS),
    "entities": lambda a, b: diff_block(a, b, 0x02D948, ENTITY_STRIDE, ENTITY_COUNT,
                                        ENTITY_DIFF_FIELDS),
    "items": lambda a, b: diff_block(a, b, 0x001148, ITEM_STRIDE, ITEM_COUNT,
                                     ITEM_DIFF_FIELDS),
    "world_objects": lambda a, b: diff_block(a, b, 0x025C48, 0x40, 500,
                                             WORLD_OBJECT_DIFF_FIELDS),
    "markers": lambda a, b: diff_block(a, b, 0x021148, 0x40, 200, ()),
    "projectiles": lambda a, b: diff_block(a, b, 0x0004C8, 0x20, 100, ()),
    "settlements": lambda a, b: diff_block(a, b, 0x0AA948, 0x4B5, 20,
                                           SETTLEMENT_DIFF_FIELDS),
    "quest_flags": diff_quest_flags,
    "discovered": diff_discovered,
}


def _fmt_value(value) -> str:
    if isinstance(value, float):
        return f"{value:g}"
    return str(value)


def _print_block(name: str, rows: list[dict], limit: int) -> None:
    if not rows:
        print(f"  {name}: no changes")
        return
    print(f"  {name}: {len(rows)} record(s)")
    for row in rows[:limit]:
        if "fields" not in row:
            print(f"    [{row['index']}] {row['before']} -> {row['after']}")
            continue
        tag = " (created)" if row["created"] else " (removed)" if row["removed"] else ""
        bits = [f"{k} {_fmt_value(v[0])} -> {_fmt_value(v[1])}"
                for k, v in row["fields"].items()]
        bits += [f"{k} 0x{v[0]:02X} -> 0x{v[1]:02X}" for k, v in row["raw"].items()]
        print(f"    [{row['index']}]{tag} " + ", ".join(bits))
    if len(rows) > limit:
        print(f"    ... {len(rows) - limit} more (raise --limit)")


def cmd_diff(proc: GameProcess | None, args) -> int:
    a = Path(args.before).read_bytes()
    b = Path(args.after).read_bytes()
    if len(a) != SAVE_SIZE or len(b) != SAVE_SIZE:
        raise CheatError(
            f"both images must be {SAVE_SIZE} bytes (0x{SAVE_SIZE:X}); got "
            f"{len(a)} and {len(b)}"
        )
    header = diff_header(a, b)
    wanted = args.blocks.split(",") if args.blocks else list(DIFF_BLOCKS)
    report: dict = {"header": header, "blocks": {}}
    for name in wanted:
        if name not in DIFF_BLOCKS:
            raise CheatError(f"unknown block '{name}' (choose from "
                             f"{', '.join(DIFF_BLOCKS)})")
        if name == "entities" and args.entity is not None:
            report["blocks"][name] = diff_block(
                a, b, 0x02D948, ENTITY_STRIDE, ENTITY_COUNT,
                ENTITY_DIFF_FIELDS, only=args.entity,
            )
        else:
            report["blocks"][name] = DIFF_BLOCKS[name](a, b)
    if args.json:
        print(json.dumps(report, ensure_ascii=False, indent=2, default=str))
        return 0
    print(f"snapshot diff {Path(args.before).name} -> {Path(args.after).name}")
    if header:
        for name, (va, vb) in header.items():
            print(f"  header {name}: {va} -> {vb}")
    else:
        print("  header: no changes")
    for name in wanted:
        _print_block(name, report["blocks"][name], args.limit)
    return 0


# ---------------------------------------------------------------------------
# Commands
# ---------------------------------------------------------------------------


def cmd_key(proc: GameProcess, args) -> int:
    vk = parse_vk(args.key)
    proc.activate()
    proc.key(vk)
    print(f"posted WM_KEYDOWN VK 0x{vk:02X} ({args.key})")
    return 0


def cmd_click(proc: GameProcess, args) -> int:
    print(f"client {'right' if args.right else 'left'}-click at ({args.x},{args.y})")
    proc.activate()
    proc.click(args.x, args.y, hold=args.hold, right=args.right)
    return 0


def cmd_hover(proc: GameProcess, args) -> int:
    """Move the game cursor and report the actor under it (`_81D578`)."""
    proc.activate()
    proc.move_mouse(args.x, args.y)
    proc.pump(0.15)
    cx, cy = proc.i32(CURSOR_X), proc.i32(CURSOR_Y)
    ptr = proc.u32(HOVERED_ACTOR)
    print(f"cursor ({cx},{cy}), actor pointer 0x{ptr:08X}")
    if ENTITIES <= ptr < ENTITIES + ENTITY_COUNT * ENTITY_STRIDE:
        idx = (ptr - ENTITIES) // ENTITY_STRIDE
        ent = decode_entity(proc.entity_bytes(idx))
        print(f"  actor: entity {idx} archetype 0x{ent['archetype']:02X} "
              f"cell ({ent['x']},{ent['y']}) dialogue 0x{ent['dialogue']:02X} "
              f"group {ent['group']} level {ent['level']}")
    else:
        print("  actor: none")
    return 0


def cmd_dialogue(proc: GameProcess, args) -> int:
    """Dump the live dialogue records (`FUN_00434C74` output lists)."""
    mode = proc.iface_mode
    npc_ptr = proc.u32(DIALOG_NPC)
    count = proc.i32(DIALOG_OPTION_COUNT)
    print(f"interface mode {mode} (1 = dialogue), option count {count}")
    if ENTITIES <= npc_ptr < ENTITIES + ENTITY_COUNT * ENTITY_STRIDE:
        idx = (npc_ptr - ENTITIES) // ENTITY_STRIDE
        ent = decode_entity(proc.entity_bytes(idx))
        print(f"dialogue NPC: entity {idx} dialogue id 0x{ent['dialogue']:02X} "
              f"name {ent['name_id']} group {ent['group']}")
    else:
        print(f"dialogue NPC: none (0x{npc_ptr:08X})")
    for i in range(max(0, min(count, 16))):
        raw = proc.read(DIALOG_OPTIONS + i * 0x20, 0x20)
        rect16 = struct.unpack_from("<4h", raw, 0)
        line = struct.unpack_from("<I", raw, 8)[0]
        print(f"  option[{i}] rect16 {rect16} line {line} raw {raw.hex(' ')}")
    if args.statements:
        # Statement records have no documented count global; print the first
        # few so a live dialogue can be inspected.
        for i in range(4):
            raw = proc.read(DIALOG_STATEMENTS + i * 0x20, 0x20)
            print(f"  stmt[{i}] raw {raw.hex(' ')}")
    return 0


VK_NAMES = {
    "esc": 0x1B, "escape": 0x1B, "space": 0x20, "tab": 0x09,
    "enter": 0x0D, "return": 0x0D, "shift": 0x10, "ctrl": 0x11,
    "left": 0x25, "up": 0x26, "right": 0x27, "down": 0x28,
    "home": 0x24, "end": 0x23, "pageup": 0x21, "pagedown": 0x22,
}


def parse_vk(name: str) -> int:
    key = name.strip().lower()
    if key in VK_NAMES:
        return VK_NAMES[key]
    if len(key) == 1:
        ch = key.upper()
        if "A" <= ch <= "Z" or "0" <= ch <= "9":
            return ord(ch)
    try:
        return int(key, 0)
    except ValueError:
        raise CheatError(f"unknown key '{name}' (use a letter, digit or VK number)") from None


def cmd_menu(proc: GameProcess, args) -> int:
    """Reach the main menu and press one of its six buttons."""
    proc.activate()
    for _ in range(5):
        if proc.screen_state == 1:
            break
        proc.key(0x1B)
        proc.pump(0.4)
    proc.wait_for(lambda: proc.screen_state == 1, 10, "main menu")
    click_main_menu(proc, args.button)
    time.sleep(0.4)
    print(f"menu '{args.button}' clicked (screen state {proc.screen_state})")
    return 0


def click_main_menu(proc: GameProcess, button: str) -> None:
    width = 640 if proc.logical_size[0] <= 640 else 800
    x, y, w, h = MAIN_MENU_RECTS[width][button]
    proc.click(x + w // 2, y + h // 2)


def resume_if_menu(proc: GameProcess) -> None:
    """Return to gameplay when the main menu is open (Escape opened it)."""
    if proc.screen_state != 1:
        return
    print("main menu is open: clicking Resume")
    click_main_menu(proc, "resume")
    proc.wait_for(lambda: proc.screen_state == 0, 15, "gameplay to resume")


#: Render/config globals toggled live by the settings screen
#: (`KONUNG_SOURCE_OF_TRUTH.md` §35.1/§35.5).
RENDER_TOGGLES = {
    "day_night": 0x81D5EC,
    "light_effects": 0x81D604,
    "shadows": 0x81D620,
}


def cmd_frame(proc: GameProcess, args) -> int:
    """Set the world frame (ambient phase) and the three render toggles.

    The main loop advances `WORLD_FRAME` every 60 ms frame, so the value is a
    request: the next rendered frame reads roughly the value written here plus
    the capture latency. That is exact enough for the ambient table, whose
    plateaus are hundreds of frames wide; the printed phase is what the process
    holds immediately after the write.
    """
    before = proc.world_frame
    proc.write(WORLD_FRAME, struct.pack("<I", args.value & 0xFFFFFFFF))
    for name, va in RENDER_TOGGLES.items():
        value = getattr(args, name)
        if value is not None:
            proc.write(va, struct.pack("<I", 1 if value else 0))
    after = proc.world_frame
    print(f"world frame {before} -> requested {args.value} "
          f"(now {after}, phase {after % 0x5460})")
    for name, va in RENDER_TOGGLES.items():
        value = getattr(args, name)
        if value is not None:
            print(f"{name} = {proc.u32(va)}")
    return 0


def cmd_state(proc: GameProcess, args) -> int:
    state = proc.screen_state
    loc = proc.location
    leader = proc.leader_index()
    party = proc.party_indices() if leader is not None else []
    sel = [(p - ENTITIES) // ENTITY_STRIDE for p in proc.selection()]
    data = {
        "pid": proc.pid,
        "module_base": f"0x{proc.base:X}",
        "video_mode": f"0x{proc.video_mode:X}",
        "logical_size": list(proc.logical_size),
        "screen_state": state,
        "iface_mode": proc.iface_mode,
        "location": loc,
        "location_name": location_name(proc, loc) if loc else "",
        "world_frame": proc.world_frame,
        "leader": leader,
        "selection": sel,
        "party": party,
        "discovered": sum(1 for b in proc.read(DISCOVERED, 56) if b),
    }
    if args.json:
        print(json.dumps(data, ensure_ascii=False, indent=2))
        return 0
    names = {0: "gameplay", 1: "main menu", 2: "new hero", 3: "fatal",
             4: "load", 5: "save", 6: "options", 7: "menu entry",
             8: "new-game load", 9: "intro movie"}
    print(f"pid {proc.pid}  base 0x{proc.base:X}  "
          f"video 0x{proc.video_mode:X} ({proc.logical_size[0]}x{proc.logical_size[1]})")
    print(f"screen state   : {state} ({names.get(state, '?')})")
    print(f"interface mode : {proc.iface_mode} "
          f"(0 world, 1 dialogue, 2 quest, 3 character, 5 map, 7 detail)")
    print(f"location       : {loc} ({location_name(proc, loc) if loc else '-'})")
    print(f"world frame    : {proc.world_frame}")
    if leader is None:
        print("leader         : - (no game in progress)")
    else:
        ent = decode_entity(proc.entity_bytes(leader))
        hero = HERO_NAMES.get(ent["name_id"])
        print(f"leader         : entity {leader} cell ({ent['x']},{ent['y']}) "
              f"facing {ent['facing']} kind {ent['kind']} "
              f"archetype 0x{ent['archetype']:02X} level {ent['level']} "
              f"hp {ent['hp']} ({(ent['hp'] + 15) // 16}) "
              f"money {ent['money']}"
              + (f" hero {hero}" if hero else ""))
    print(f"selection      : {sel if sel else '-'}")
    print(f"party (group 0): {party if party else '-'}")
    print(f"map discovered : {data['discovered']}/56")
    return 0


def cmd_locations(proc: GameProcess, args) -> int:
    discovered = proc.read(DISCOVERED, 56)
    points = map_points(proc)
    reachable = transition_destinations(proc)
    rows = []
    for loc in range(1, LOCATION_NAME_COUNT + 1):
        name = location_name(proc, loc)
        pt = points.get(loc)
        rows.append({
            "location": loc,
            "name": name,
            "map_point": list(pt) if pt else None,
            "discovered": bool(discovered[loc - 1]),
            "reserved": (loc - 1) in RESERVED_MASK,
            "current": loc == proc.location,
            "transition": loc in reachable,
        })
    if args.json:
        print(json.dumps(rows, ensure_ascii=False, indent=2))
        return 0
    print(f"{'id':>3}  {'disc':>4}  {'resv':>4}  {'trans':>5}  {'map point':>12}  name")
    for r in rows:
        mark = "*" if r["current"] else " "
        pt = f"{r['map_point'][0]},{r['map_point'][1]}" if r["map_point"] else "-"
        print(f"{r['location']:>3}{mark} {str(r['discovered']):>5}  "
              f"{str(r['reserved']):>4}  {str(r['transition']):>5}  {pt:>12}  {r['name']}")
    print("disc = revealed flag, trans = a transition record targets this location")
    return 0


def map_rows(proc: GameProcess) -> list[list[tuple[int, int]]]:
    offsets = struct.unpack("<361I", proc.read(MAP_ROWS_VA, MAP_ROWS * 4))
    blob = proc.read(MAP_RUNS_VA, max(offsets) + 512)
    rows = []
    for row_off in offsets:
        p = row_off
        runs = []
        for _ in range(600):
            loc = blob[p]
            width = blob[p + 1]
            p += 2
            runs.append((loc, width))
            if width == 0:
                break
        rows.append(runs)
    return rows


def map_points(proc: GameProcess) -> dict[int, tuple[int, int]]:
    """Location id (1-based) -> map-local click pixel.

    The run table stores **0-based** run values (`run = location - 1`); the
    command compiler returns `run*0x100|0x103` and the click handler's `>>8`
    yields `run + 1`, the 1-based location id compared with `_DAT_0081D6C4`
    and with the transition `dst`. The discovered mask is indexed by the run
    value, i.e. by `location - 1`.
    """
    rows = map_rows(proc)
    found: dict[int, tuple[int, int]] = {}
    for y, runs in enumerate(rows):
        x = 0
        for loc, width in runs:
            if width == 0:
                break
            if loc < LOCATION_NAME_COUNT:
                loc_id = loc + 1
                if loc_id not in found:
                    found[loc_id] = (x + width // 2, y)
            x += width
    return found


def map_click_point(proc: GameProcess, loc_id: int) -> tuple[int, int] | None:
    """Client pixel of a location's map region (viewport + overlay offset)."""
    point = map_points(proc).get(loc_id)
    if point is None:
        return None
    width = proc.logical_size[0]
    dx, dy = (13, 13) if width <= 640 else (80, 60)
    left = proc.i32(VIEW_LEFT)
    top = proc.i32(VIEW_TOP)
    return (left + dx + point[0], top + dy + point[1])


def cmd_entities(proc: GameProcess, args) -> int:
    loc = args.loc if args.loc is not None else proc.location
    groups = proc.groups()
    leader = proc.leader_index()
    party = set(proc.party_indices())
    rows = []
    for g in groups:
        if g["count"] == 0:
            continue
        if not args.all and g["loc_id"] != loc:
            continue
        for e in range(g["first_entity"], g["first_entity"] + g["count"]):
            if e >= ENTITY_COUNT:
                break
            ent = decode_entity(proc.entity_bytes(e))
            rows.append({
                "entity": e,
                "group": g["index"],
                "loc": g["loc_id"],
                "cell": [ent["x"], ent["y"]],
                "kind": ent["kind"],
                "archetype": f"0x{ent['archetype']:02X}",
                "level": ent["level"],
                "hp": ent["hp"],
                "money": ent["money"],
                "leader": e == leader,
                "party": e in party,
            })
    if args.json:
        print(json.dumps(rows, ensure_ascii=False, indent=2))
        return 0
    print(f"location {loc} ({location_name(proc, loc)}): {len(rows)} entities")
    print(f"{'ent':>5} {'grp':>4} {'kind':>4} {'arch':>5} {'cell':>10} "
          f"{'lvl':>4} {'hp':>6} {'money':>8}  flags")
    for r in rows:
        flags = []
        if r["leader"]:
            flags.append("LEADER")
        if r["party"]:
            flags.append("party")
        print(f"{r['entity']:>5} {r['group']:>4} {r['kind']:>4} {r['archetype']:>5} "
              f"{r['cell'][0]:>4},{r['cell'][1]:<5} {r['level']:>4} {r['hp']:>6} "
              f"{r['money']:>8}  {' '.join(flags)}")
    return 0


def cmd_dump(proc: GameProcess, args) -> int:
    index = proc.resolve_entity(args.entity)
    ent = decode_entity(proc.entity_bytes(index))
    table = instance_table(proc)
    catalog = ItemCatalog(proc.read)
    if args.json:
        print(json.dumps({"entity": index, **ent}, ensure_ascii=False, indent=2))
        return 0
    leader = proc.leader_index()
    print(f"entity {index}  ptr 0x{proc.entity_ptr(index):08X}  "
          f"group {ent['group']}  "
          f"{'LEADER' if index == leader else ''}")
    print(f"  cell      : ({ent['x']},{ent['y']})  dest ({ent['dest_x']},{ent['dest_y']})")
    print(f"  identity  : kind {ent['kind']} facing {ent['facing']} "
          f"archetype 0x{ent['archetype']:02X} visual {ent['visual']} "
          f"palette 0x{ent['palette']:X} name {ent['name_id']} "
          f"nick {ent['nickname_id']} dialogue 0x{ent['dialogue']:02X}")
    print(f"  order     : 0x{ent['order']:02X} flags 0x{ent['flags']:02X} "
          f"target 0x{ent['target']:02X} weapon slot {ent['weapon_slot']}")
    print(f"  level/exp : {ent['level']}  xp {ent['xp']} invested {ent['invested_xp']} "
          f"money {ent['money']}")
    print(f"  hp        : {ent['hp']} internal = {(ent['hp'] + 15) // 16} display "
          f"(max 100 / 1600 internal)")
    print(f"  stats     : str {ent['str']}/{ent['str_cur']}  dex {ent['dex']}/{ent['dex_cur']} "
          f"cha {ent['cha']}/{ent['cha_cur']}  end {ent['end']}/{ent['end_cur']}")
    print(f"  skills    : melee {ent['melee']} archery {ent['archery']} trade {ent['trade']} "
          f"building {ent['building']} healing {ent['healing']} identify {ent['identify']} "
          f"smithing {ent['smithing']}")
    print(f"  armor     : {ent['armor']}  poison {ent['poison']}  ammo id {ent['special']}")
    print(f"  effects   : id {ent['effect_id']} timer {ent['effect_timer']}  "
          f"saved str/dex/cha/end "
          f"{ent['saved_str']}/{ent['saved_dex']}/{ent['saved_cha']}/{ent['saved_end']}  "
          f"state 0x{ent['state_d2']:02X}/0x{ent['state_d3']:02X}")
    print("  equipment:")
    for i, item in enumerate(ent["equipment"]):
        print(f"    [{i}] {format_item(catalog, decode_instance(table, item))}")
    print("  additional:")
    for i, item in enumerate(ent["additional"]):
        print(f"    [{i}] {format_item(catalog, decode_instance(table, item))}")
    print("  inventory:")
    for i, item in enumerate(ent["inventory"]):
        if item:
            print(f"    [{i:>2}] {format_item(catalog, decode_instance(table, item))}")
    return 0


def cmd_set(proc: GameProcess, args) -> int:
    index = proc.resolve_entity(args.entity)
    changes = []
    for assignment in args.assignments:
        for op in ("+=", "-=", "="):
            if op in assignment:
                field, _, value = assignment.partition(op)
                break
        else:
            raise CheatError(f"expected field=value, got '{assignment}'")
        field = field.strip()
        value = int(value.strip(), 0)
        if field == "hp_display":
            value = value * 16 - 15
            field = "hp"
        canonical = ENTITY_ALIASES.get(field, field)
        if canonical not in ENTITY_FIELDS:
            raise CheatError(
                f"unknown field '{field}' (fields: {', '.join(sorted(ENTITY_FIELDS))})"
            )
        if op == "+=":
            value = proc.entity_field(index, canonical) + value
        elif op == "-=":
            value = proc.entity_field(index, canonical) - value
        if canonical == "hp":
            # The original's health ceiling is a fixed 1600 internal units
            # (= 100 display): potion restores use `min(1600, ...)`, the revive
            # path writes 1600, and opening the character screen clamps
            # `Entity+0x5C` to 1600 (verified live). Writing more only looks
            # right until the next screen open.
            if value > 1600:
                print(f"note: health is capped at 1600 internal (100 display); "
                      f"clamping {value} -> 1600")
                value = 1600
            value = max(0, value)
        before = proc.entity_field(index, canonical)
        proc.set_entity_field(index, canonical, value)
        changes.append((canonical, before, proc.entity_field(index, canonical)))
    for field, before, after in changes:
        print(f"entity {index} {field}: {before} -> {after}")
    return 0


def cmd_teleport(proc: GameProcess, args) -> int:
    index = proc.resolve_entity(args.entity)
    x, y = args.x, args.y
    if not (0 <= x < 320 and 0 <= y < 160):
        raise CheatError("cell coordinates are x 0..319, y 0..159 (320x160 grid)")
    ptr = proc.entity_ptr(index)
    proc.put_u16(ptr + 0x08, x)
    proc.put_u16(ptr + 0x0A, y)
    proc.put_u16(ptr + 0x0C, x)
    proc.put_u16(ptr + 0x0E, y)
    # Clear the direction queue terminator (no pending walk segments).
    proc.write(ptr + 0x13, b"\xff\xff")
    if args.facing is not None:
        proc.put_u8(ptr + 0x01, args.facing & 7)
    if args.halt:
        proc.put_u8(ptr + 0x02, 0x60)
        proc.put_u8(ptr + 0x00, 0)
    print(f"entity {index} -> cell ({x},{y})"
          + (f" facing {args.facing}" if args.facing is not None else ""))
    return 0


def find_free_instance(table: bytes) -> int:
    for item_id in range(1, ITEM_COUNT):
        rec = table[item_id * ITEM_STRIDE:(item_id + 1) * ITEM_STRIDE]
        if rec == b"\xff" * ITEM_STRIDE:
            return item_id
    for item_id in range(1, ITEM_COUNT):
        if table[item_id * ITEM_STRIDE] == 0xFF:
            return item_id
    raise CheatError("item-instance table is full")


def find_template_instance(table: bytes, class_id: int, subtype: int) -> bytes | None:
    for item_id in range(1, ITEM_COUNT):
        rec = table[item_id * ITEM_STRIDE:(item_id + 1) * ITEM_STRIDE]
        if rec[0] == class_id and rec[3] == subtype and rec[0] != 0xFF:
            return rec
    return None


def template_from_files(install: Path | None, class_id: int, subtype: int) -> bytes | None:
    if install is None:
        return None
    for name in ("Game.0", "Game.1", "Game.2"):
        path = install / name
        if not path.exists():
            continue
        data = path.read_bytes()
        for item_id in range(1, ITEM_COUNT):
            o = item_id * ITEM_STRIDE
            rec = data[o:o + ITEM_STRIDE]
            if len(rec) < ITEM_STRIDE:
                break
            if rec[0] == class_id and rec[3] == subtype:
                return rec
    return None


def build_item_record(class_id: int, subtype: int, template: bytes | None,
                      condition: int, modifier: int, quantity: int | None,
                      durability: float | None, poison: int | None) -> bytes:
    if template is not None:
        rec = bytearray(template)
        if rec[0] == 0:
            rec = bytearray(ITEM_STRIDE)
            rec[0] = class_id
    else:
        rec = bytearray(ITEM_STRIDE)
        rec[0] = class_id
        # Classes without durability/quantity carry 0xFFFFFFFF in +4/+8.
        if class_id in (6, 7, 8, 11):
            struct.pack_into("<I", rec, 4, 0xFFFFFFFF)
            struct.pack_into("<I", rec, 8, 0xFFFFFFFF)
        else:
            struct.pack_into("<f", rec, 4, 100.0)
            struct.pack_into("<f", rec, 8, 100.0)
    rec[0] = class_id
    rec[3] = subtype
    if condition is not None:
        rec[1] = condition & 0xFF
    if modifier is not None:
        rec[2] = modifier & 0xFF
    if quantity is not None:
        if class_id == 9:
            struct.pack_into("<f", rec, 4, float(quantity))
        else:
            struct.pack_into("<I", rec, 4, quantity & 0xFFFFFFFF)
    if durability is not None:
        struct.pack_into("<f", rec, 4, float(durability))
        struct.pack_into("<f", rec, 8, float(durability))
    if poison is not None:
        struct.pack_into("<H", rec, 12, poison & 0xFFFF)
    return bytes(rec)


def allocate_item(proc: GameProcess, class_id: int, subtype: int, *,
                  install: Path | None, condition: int = -1, modifier: int = -1,
                  quantity: int | None = None, durability: float | None = None,
                  poison: int = 0, table: bytes | None = None,
                  catalog: ItemCatalog | None = None) -> int:
    """Write a free item instance for ``(class, subtype)`` and return its id.

    The instance is cloned from an existing live/on-disk instance of the same
    prototype when one exists (so durability/stack size/flags are the values the
    game itself generates), otherwise a neutral record is synthesised.
    """
    if class_id not in ITEM_TABLES:
        raise CheatError(f"item class {class_id} has no prototype table")
    catalog = catalog or ItemCatalog(proc.read)
    info = catalog.info(class_id, subtype)
    if info is None:
        raise CheatError(
            f"class {class_id} subtype {subtype} is not a shipped item prototype "
            f"(try 'catalog --class {class_id}')"
        )
    table = table if table is not None else instance_table(proc)
    template = find_template_instance(table, class_id, subtype)
    if template is None:
        template = template_from_files(install, class_id, subtype)
    if quantity is None and class_id in (9, 12):
        if template is None or template[4:8] == b"\0\0\0\0":
            quantity = 1
    if durability is None and class_id not in (6, 7, 8, 9, 11, 12):
        durability = float(info["durability"] or 100)
    record = build_item_record(class_id, subtype, template, condition, modifier,
                               quantity, durability, poison)
    new_id = find_free_instance(table)
    proc.write(ITEMS + new_id * ITEM_STRIDE, record)
    return new_id


def attach_item(proc: GameProcess, index: int, ent: dict, new_id: int,
                class_id: int, where: str = "auto", slot: int | None = None
                ) -> tuple[str, int]:
    """Store instance ``new_id`` in the entity's container; returns (label, old)."""
    ptr = proc.entity_ptr(index)
    if where == "auto":
        # Prefer the item's own equipment slot when it is empty; never silently
        # replace something the character already wears.
        if class_id in (0, 1, 2, 3, 4, 5) and ent["equipment"][class_id] == 0:
            where = "equip"
        elif class_id == 12 and ent["special"] == 0:
            where = "ammo"
        elif class_id in (6, 7, 8) and 0 in ent["additional"]:
            where = "additional"
        else:
            where = "inventory"
    if where == "equip":
        slot = class_id if slot is None else slot
        if not 0 <= slot < MAIN_SLOTS:
            raise CheatError(f"equipment slot {slot} outside 0..{MAIN_SLOTS - 1}")
        old = ent["equipment"][slot]
        proc.put_u16(ptr + EQUIP_OFF + slot * 2, new_id)
        return f"equipment[{slot}]", old
    if where == "additional":
        if slot is None:
            slot = next((i for i, v in enumerate(ent["additional"]) if v == 0), None)
            if slot is None:
                raise CheatError("all five additional slots are occupied (--slot N)")
        if not 0 <= slot < ADDITIONAL_SLOTS:
            raise CheatError(f"additional slot {slot} outside 0..{ADDITIONAL_SLOTS - 1}")
        old = ent["additional"][slot]
        proc.put_u16(ptr + ADDITIONAL_OFF + slot * 2, new_id)
        return f"additional[{slot}]", old
    if where == "ammo":
        old = ent["special"]
        proc.put_u16(ptr + SPECIAL_OFF, new_id)
        return "ammo", old
    if slot is None:
        slot = next((i for i, v in enumerate(ent["inventory"]) if v == 0), None)
        if slot is None:
            raise CheatError("inventory is full (pass --slot N to replace one)")
    if not 0 <= slot < INVENTORY_SLOTS:
        raise CheatError(f"inventory slot {slot} outside 0..{INVENTORY_SLOTS - 1}")
    old = ent["inventory"][slot]
    proc.put_u16(ptr + INVENTORY_OFF + slot * 2, new_id)
    return f"inventory[{slot}]", old


def cmd_give(proc: GameProcess, args) -> int:
    index = proc.resolve_entity(args.entity)
    install = Path(args.install) if args.install else proc.install_dir
    catalog = ItemCatalog(proc.read)
    info = catalog.info(args.class_id, args.subtype)
    if info is None:
        raise CheatError(
            f"class {args.class_id} subtype {args.subtype} is not a shipped item "
            f"prototype (try 'catalog --class {args.class_id}')"
        )
    new_id = allocate_item(
        proc, args.class_id, args.subtype, install=install,
        condition=args.condition, modifier=args.modifier, quantity=args.qty,
        durability=args.durability, poison=args.poison, catalog=catalog,
    )
    ent = decode_entity(proc.entity_bytes(index))
    label, old = attach_item(proc, index, ent, new_id, args.class_id,
                             args.to, args.slot)
    if old:
        print(f"replaced {label} id {old} (now orphaned)")
    print(f"entity {index} {label} := id{new_id} class {args.class_id} subtype "
          f"{args.subtype} '{info['name']}'")
    return 0


CONTAINER_OFFSET = {
    "equip": EQUIP_OFF,
    "inv": INVENTORY_OFF,
    "add": ADDITIONAL_OFF,
    "ammo": SPECIAL_OFF,
}
CONTAINER_SIZE = {
    "equip": MAIN_SLOTS,
    "inv": INVENTORY_SLOTS,
    "add": ADDITIONAL_SLOTS,
    "ammo": 1,
}


def container_slot_ptr(ptr: int, where: str, slot: int) -> int:
    base = ptr + CONTAINER_OFFSET[where]
    return base if where == "ammo" else base + slot * 2


def cmd_remove(proc: GameProcess, args) -> int:
    index = proc.resolve_entity(args.entity)
    ptr = proc.entity_ptr(index)
    where = args.where
    if where == "all":
        targets = [(w, i) for w in ("equip", "inv", "add")
                   for i in range(CONTAINER_SIZE[w])] + [("ammo", 0)]
    elif args.all:
        targets = [(where, i) for i in range(CONTAINER_SIZE[where])]
    else:
        if not 0 <= args.slot < CONTAINER_SIZE[where]:
            raise CheatError(f"{where} slot {args.slot} outside "
                             f"0..{CONTAINER_SIZE[where] - 1}")
        targets = [(where, args.slot)]
    dropped = []
    for w, s in targets:
        addr = container_slot_ptr(ptr, w, s)
        old = proc.u16(addr)
        if old == 0:
            continue
        proc.put_u16(addr, 0)
        if args.purge:
            proc.put_u8(ITEMS + old * ITEM_STRIDE, 0xFF)
        dropped.append((w, s, old))
    if not dropped:
        print(f"entity {index}: nothing to remove")
        return 0
    if len(dropped) == 1:
        w, s, old = dropped[0]
        print(f"entity {index} {w}[{s}] dropped id{old}"
              + (" (instance purged)" if args.purge else ""))
    else:
        print(f"entity {index}: cleared {len(dropped)} slots"
              + (" (instances purged)" if args.purge else ""))
        for w, s, old in dropped:
            print(f"  {w}[{s}] id{old}")
    return 0


def cmd_items(proc: GameProcess, args) -> int:
    index = proc.resolve_entity(args.entity)
    ent = decode_entity(proc.entity_bytes(index))
    table = instance_table(proc)
    catalog = ItemCatalog(proc.read)
    if args.json:
        out = {"entity": index, "slots": []}
        for where, slot, item in slot_iids(ent):
            rec = decode_instance(table, item)
            out["slots"].append({"where": where, "slot": slot,
                                 "instance": rec,
                                 "name": (catalog.info(rec["class"], rec["subtype"]) or {}).get("name", "")
                                 if rec else ""})
        print(json.dumps(out, ensure_ascii=False, indent=2))
        return 0
    print(f"entity {index} items")
    for where, slot, item in slot_iids(ent):
        if item == 0 and where == "inv":
            continue
        print(f"  {where:>5}[{slot:>2}] {format_item(catalog, decode_instance(table, item))}")
    return 0


def cmd_catalog(proc: GameProcess | None, args) -> int:
    if proc is not None:
        read = proc.read
    else:
        install = Path(args.install) if args.install else default_install()
        if install is None:
            raise CheatError("no konung.res found; pass --install DIR")
        image = PeImage(install / "konung.res")
        read = image.read
    catalog = ItemCatalog(read)
    classes = [args.class_id] if args.class_id is not None else sorted(ITEM_TABLES)
    rows = []
    for class_id in classes:
        for info in catalog.entries(class_id):
            if args.subtype is not None and info["subtype"] != args.subtype:
                continue
            if args.search and args.search.casefold() not in info["name"].casefold():
                continue
            rows.append(info)
    if args.json:
        print(json.dumps(rows, ensure_ascii=False, indent=2))
        return 0
    print(f"{'cls':>3} {'sub':>4} {'weight':>7} {'rating':>7} {'price':>6} "
          f"{'dur':>4} {'icon':>5}  name")
    for r in rows:
        print(f"{r['class']:>3} {r['subtype']:>4} {r['weight']:>7} {r['rating']:>7} "
              f"{r['price']:>6} {r['durability']:>4} {r['icon']:>5}  {r['name']}")
    return 0


def cmd_discover(proc: GameProcess, args) -> int:
    before = proc.read(DISCOVERED, 56)
    proc.write(DISCOVERED, b"\xff" * 56)
    print(f"discovered locations: {sum(1 for b in before if b)}/56 -> 56/56")
    return 0


def cmd_travel(proc: GameProcess, args) -> int:
    target = args.location
    if not 1 <= target <= LOCATION_NAME_COUNT:
        raise CheatError(f"location id must be 1..{LOCATION_NAME_COUNT}")
    current = proc.location
    if current == target:
        print(f"already at location {target} ({location_name(proc, target)})")
        return 0
    if proc.screen_state != 0:
        if proc.screen_state == 1:
            resume_if_menu(proc)
        if proc.screen_state != 0:
            raise CheatError(
                f"game is not in gameplay (screen state {proc.screen_state}); "
                f"start/load a game first"
            )
    # The original validates against the loaded transition table: a click only
    # travels when some record's destination (+0x04) equals the clicked map
    # index. Otherwise it silently does nothing (and still eats the orb), so
    # check first.
    reachable = transition_destinations(proc)
    if target not in reachable:
        listing = ", ".join(f"{d} ({location_name(proc, d)})"
                            for d in sorted(reachable) if d <= LOCATION_NAME_COUNT)
        raise CheatError(
            f"no transition record targets location {target} "
            f"({location_name(proc, target)}); map travel can only reach: {listing}"
        )
    if not args.no_reveal:
        proc.write(DISCOVERED, b"\xff" * 56)
    if proc.iface_mode not in (0, 5):
        proc.key(0x1B)
        proc.wait_for(lambda: proc.iface_mode == 0, 5, "interface window to close")
    leader = proc.leader_index()
    if leader is None:
        raise CheatError("no live leader")
    ent = decode_entity(proc.entity_bytes(leader))
    if not args.no_orb and not leader_has_orb(proc, ent):
        new_id = allocate_item(proc, TRAVEL_ORB_CLASS, TRAVEL_ORB_SUBTYPE,
                               install=Path(args.install) if args.install
                               else proc.install_dir,
                               catalog=ItemCatalog(proc.read))
        label, old = attach_item(proc, leader, ent, new_id,
                                 TRAVEL_ORB_CLASS, "inventory")
        print(f"added a travel orb ('Шар магического прыжка', id{new_id}) to "
              f"{label}; the original consumes one per map travel")
        ent = decode_entity(proc.entity_bytes(leader))
    if proc.iface_mode != 5:
        proc.key(0x4D)  # 'M' toggles the world map
        proc.wait_for(lambda: proc.iface_mode == 5, 5, "world map to open")
    point = (args.point if args.point else map_click_point(proc, target))
    if point is None:
        raise CheatError(
            f"location {target} has no map region (reserved locations are not "
            f"drawn); use 'locations' to inspect"
        )
    x, y = point
    print(f"travelling to {target} ({location_name(proc, target)}) "
          f"via map click at client ({x},{y})")
    proc.click(x, y)
    try:
        proc.wait_for(lambda: proc.location == target, args.timeout,
                      f"location {target} to load")
    except CheatError:
        if proc.iface_mode == 5:
            proc.key(0x1B)  # Escape closes the map
        raise
    proc.wait_for(lambda: proc.screen_state == 0, 20, "gameplay to resume")
    leader = proc.leader_index()
    cell = None
    if leader is not None:
        ent = decode_entity(proc.entity_bytes(leader))
        cell = (ent["x"], ent["y"])
    print(f"arrived: location {proc.location} ({location_name(proc, proc.location)})"
          + (f", leader cell {cell}" if cell else ""))
    return 0


def transition_destinations(proc: GameProcess) -> set[int]:
    """Destination location ids present in the runtime transition table."""
    count = proc.u32(TRANSITION_COUNT)
    if not 0 < count <= 250:
        return set()
    raw = proc.read(TRANSITIONS, count * TRANSITION_STRIDE)
    return {raw[i * TRANSITION_STRIDE + 4] for i in range(count)
            if raw[i * TRANSITION_STRIDE + 4] != 0}


def leader_has_orb(proc: GameProcess, ent: dict) -> bool:
    table = instance_table(proc)
    for item in ent["inventory"]:
        inst = decode_instance(table, item)
        if inst and inst["class"] == TRAVEL_ORB_CLASS and \
                inst["subtype"] == TRAVEL_ORB_SUBTYPE:
            return True
    return False


def cmd_snapshot(proc: GameProcess, args) -> int:
    data = build_snapshot(proc)
    path = Path(args.path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)
    print(f"wrote {path} ({len(data)} bytes)")
    info = snapshot_summary(data)
    print(f"  leader entity {info['leader_index']} cell {info['leader_cell']} "
          f"level {info['leader_level']} money {info['leader_money']}")
    print(f"  world frame {info['world_frame']}, "
          f"{info['discovered']}/56 locations discovered")
    if args.slot is not None:
        install = Path(args.install) if args.install else proc.install_dir
        if install is None:
            raise CheatError("cannot determine the install dir; pass --install")
        slot_path = install / f"KONUNG.SA{args.slot}"
        slot_path.write_bytes(data)
        print(f"  installed as {slot_path}")
        if args.name:
            write_save_name_table(install, args.slot, args.name)
            print(f"  KONUNG.SAV slot {args.slot} named '{args.name}'")
    return 0


def cmd_load_save(proc: GameProcess, args) -> int:
    source = Path(args.path)
    if not source.exists():
        raise CheatError(f"{source} does not exist")
    data = source.read_bytes()
    if len(data) != SAVE_SIZE:
        raise CheatError(
            f"{source} is {len(data)} bytes; a KONUNG.SAn image is {SAVE_SIZE} "
            f"(0x{SAVE_SIZE:X})"
        )
    install = Path(args.install) if args.install else proc.install_dir
    if install is None:
        raise CheatError("cannot determine the install dir; pass --install")
    slot = args.slot
    target = install / f"KONUNG.SA{slot}"
    target.write_bytes(data)
    # Keep any existing slot name; write one only when asked (or when the slot
    # is still unnamed, so the Load screen shows something identifiable).
    name = args.name
    if name is None and not save_slot_name(install, slot):
        name = source.stem
    if name:
        write_save_name_table(install, slot, name)
    print(f"installed {source} -> {target}"
          + (f" (slot name '{name}')" if name else ""))
    if not args.apply:
        print("open the Load screen (Escape -> Load) and pick the slot, "
              "or rerun with --apply")
        return 0
    # Drive the menu: Escape -> main menu, Load button, slot rectangle.
    for _ in range(4):
        if proc.screen_state == 1:
            break
        proc.key(0x1B)
        proc.pump(0.4)
    proc.wait_for(lambda: proc.screen_state == 1, 10, "main menu")
    w = 640 if proc.logical_size[0] <= 640 else 800
    x, y, rw, rh = MAIN_MENU_RECTS[w]["load"]
    proc.click(x + rw // 2, y + rh // 2)
    proc.wait_for(lambda: proc.screen_state == 4, 10, "load screen")
    x, y, rw, rh = SAVE_SLOT_RECTS[w][slot]
    proc.click(x + rw // 2, y + rh // 2)
    proc.wait_for(lambda: proc.screen_state == 0, args.timeout, "snapshot load")
    leader = proc.leader_index()
    print(f"loaded slot {slot}: location {proc.location} "
          f"({location_name(proc, proc.location)}), leader entity {leader}")
    return 0


def cmd_new_game(proc: GameProcess, args) -> int:
    if args.keep and proc.leader_index() is not None and proc.screen_state == 0:
        print("a game is already running (--keep)")
        return 0
    hero = args.hero
    if not 0 <= hero <= 2:
        raise CheatError("--hero must be 0 (Dragomir), 1 (Mikhail) or 2 (Wolf)")
    class_id = CLASS_ALIASES[args.class_name]
    # Leave gameplay/other screens until the main menu (state 1) is reached.
    for _ in range(6):
        try:
            state = proc.screen_state
        except CheatError:
            state = -1
        if state == 1:
            break
        if state == 9 and not args.skip_intro:
            print("skipping the intro movie (pass --wait-intro to watch it)")
        proc.key(0x1B)
        proc.pump(0.5)
    proc.wait_for(lambda: proc.screen_state == 1, 20, "main menu")
    w = 640 if proc.logical_size[0] <= 640 else 800
    layout = NEWH_RECTS[w]
    x, y, rw, rh = MAIN_MENU_RECTS[w]["new"]
    proc.click(x + rw // 2, y + rh // 2)
    proc.wait_for(lambda: proc.screen_state == 2, 10, "new hero screen")
    x, y, rw, rh = layout["hero"][hero]
    proc.click(x + rw // 2, y + rh // 2)
    proc.pump(0.2)
    x, y, rw, rh = layout["class"][class_id]
    proc.click(x + rw // 2, y + rh // 2)
    proc.pump(0.2)
    x, y, rw, rh = layout["ok"]
    proc.click(x + rw // 2, y + rh // 2)
    proc.wait_for(lambda: proc.screen_state == 0, args.timeout, "new game to start")
    proc.pump(0.5)
    leader = proc.leader_index()
    if leader is None:
        raise CheatError("new game started but the leader pointer is still empty")
    ent = decode_entity(proc.entity_bytes(leader))
    hero_name = ("Dragomir", "Mikhail", "Wolf")[hero]
    print(f"new game: hero {hero} ({hero_name}), "
          f"class {class_id} ({CLASS_NAMES[class_id]})")
    print(f"leader entity {leader} cell ({ent['x']},{ent['y']}) "
          f"location {proc.location} ({location_name(proc, proc.location)})")
    return 0


def cmd_selftest(args) -> int:
    install = Path(args.install) if args.install else default_install()
    if install is None:
        raise CheatError("no konung.res found; pass --install DIR")
    image = PeImage(install / "konung.res")
    failures = []

    # 1. Save layout must cover exactly the KONUNG.SAn size.
    end = max(off + size for off, size, _, _ in SAVE_BLOCKS)
    if end != SAVE_SIZE:
        failures.append(f"save layout ends at 0x{end:X}, expected 0x{SAVE_SIZE:X}")

    # 2. Location names (entry 0 = location 1).
    names = []
    for i in range(LOCATION_NAME_COUNT):
        raw = image.read(LOCATION_NAMES_VA + i * LOCATION_NAME_STRIDE,
                         LOCATION_NAME_STRIDE)
        names.append(raw.split(b"\0")[0].decode("cp866", "replace").strip())
    if names[0] != "Родники":
        failures.append(f"location name 0 = {names[0]!r}, expected 'Родники'")
    if names[3] != "Старый лагерь":
        failures.append(f"location name 3 = {names[3]!r}, expected 'Старый лагерь'")

    # 3. World-map run table: 361 rows; declared run widths never exceed the
    #    502 px mask. The final run of a row extends to the right edge (the
    #    original `FUN_00428390` blits `x..MAP_W` with the last location), so the
    #    declared widths may sum to less than MAP_W.
    offsets = struct.unpack("<361I", image.read(MAP_ROWS_VA, MAP_ROWS * 4))
    blob = image.read(MAP_RUNS_VA, max(offsets) + 512)
    widths = []
    for row_off in offsets:
        total = 0
        p = row_off
        seen = 0
        while seen < 600:
            width = blob[p + 1]
            p += 2
            seen += 1
            total += width
            if width == 0:
                break
        if total > MAP_W or total == 0:
            failures.append(f"map row width {total}, expected 1..{MAP_W}")
            break
        widths.append(total)

    # 4. Item prototypes resolve with CP866 names.
    catalog = ItemCatalog(image.read)
    knife = catalog.info(2, 18)
    if not knife or knife["name"] != "Нож":
        failures.append(f"class 2 subtype 18 = {knife and knife['name']!r}, expected 'Нож'")
    arrows = catalog.info(12, 30)
    if not arrows or not arrows["name"]:
        failures.append("class 12 subtype 30 has no name")
    total_items = sum(len(catalog.entries(c)) for c in ITEM_TABLES)
    if total_items < 300:
        failures.append(f"only {total_items} item prototypes resolved")

    # 5. Item record round-trip (class 12 stack).
    rec = build_item_record(12, 30, None, -1, -1, 77, None, None)
    inst = decode_instance(rec * 1, 0)
    if inst is not None:
        failures.append("instance 0 must decode as empty")
    table = b"\xff" * ITEM_STRIDE + rec
    inst = decode_instance(table, 1)
    if inst["quantity"] != 77 or inst["class"] != 12:
        failures.append("class-12 quantity round-trip failed")

    if failures:
        print("SELFTEST FAILED")
        for f in failures:
            print("  - " + f)
        return 1
    print(f"SELFTEST OK ({install})")
    print(f"  konung.res image base 0x{image.image_base:X}, "
          f"{len(image.sections)} sections")
    print(f"  location names: {names[0]!r} .. {names[3]!r}")
    print(f"  map rows: {len(widths)} x {MAP_W}px")
    print(f"  item prototypes: {total_items}")
    return 0


def default_install() -> Path | None:
    """Locate the original install (raw files) for offline commands."""
    here = Path(__file__).resolve().parent
    try:
        sys.path.insert(0, str(here))
        import assets  # type: ignore

        result = assets.resolve(here.parent)
        if result.get("base"):
            return Path(result["base"])
    except Exception:
        pass
    for candidate in (here.parent.parent, here.parent, here.parent / "assets"):
        if (candidate / "konung.res").exists():
            return candidate
    return None


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------


def parser() -> argparse.ArgumentParser:
    common = argparse.ArgumentParser(add_help=False)
    common.add_argument("--pid", type=int, default=None,
                        help="konung.res process id (default: the only one)")
    common.add_argument("--install", type=Path, default=None,
                        help="original install directory (default: the process image dir)")
    common.add_argument("--json", action="store_true", help="machine-readable output")

    ap = argparse.ArgumentParser(
        prog="original_cheat.py",
        description="Live cheat engine / scenario adapter for the original Konung.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__.split("Examples::")[1] if "Examples::" in __doc__ else None,
    )
    sub = ap.add_subparsers(dest="command", required=True)

    def add(name, func, help_text, needs_proc=True):
        p = sub.add_parser(name, parents=[common], help=help_text)
        p.set_defaults(func=func, needs_proc=needs_proc)
        return p

    add("state", cmd_state, "screen state, location, leader and party")
    p = add("frame", cmd_frame,
            "set the world frame (ambient phase) and render toggles")
    p.add_argument("value", type=int, help="new WORLD_FRAME (0x81D640)")
    p.add_argument("--day-night", type=int, choices=(0, 1), default=None,
                   help="0x81D5EC day/night cycle")
    p.add_argument("--light-effects", type=int, choices=(0, 1), default=None,
                   help="0x81D604 ambient/light masks")
    p.add_argument("--shadows", type=int, choices=(0, 1), default=None,
                   help="0x81D620 ground shadows")
    p = add("menu", cmd_menu, "click a main-menu button (resume/load/new/...)")
    p.add_argument("button", choices=sorted(MAIN_MENU_RECTS[800]))
    p = add("key", cmd_key, "post a key press to the game window")
    p.add_argument("key", help="letter, digit, esc/space/tab/enter or VK number")
    p = add("click", cmd_click, "synthesise a click at client coordinates")
    p.add_argument("x", type=int)
    p.add_argument("y", type=int)
    p.add_argument("--right", action="store_true", help="right button")
    p.add_argument("--hold", type=float, default=0.14,
                   help="how long the button stays down (seconds, default 0.14)")
    p = add("hover", cmd_hover, "move the cursor and report the actor under it")
    p.add_argument("x", type=int)
    p.add_argument("y", type=int)
    p = add("dialogue", cmd_dialogue, "dump the live dialogue option records")
    p.add_argument("--statements", action="store_true",
                   help="also dump the first NPC statement records")
    p = add("locations", cmd_locations, "location names, map points, discovery")
    p = add("entities", cmd_entities, "list entities of a location (or --all)")
    p.add_argument("--loc", type=int, default=None, help="location id (default: current)")
    p.add_argument("--all", action="store_true", help="every group of every location")

    p = add("dump", cmd_dump, "dump one entity (stats, equipment, inventory)")
    p.add_argument("entity", help="leader | selN | partyN | entity index")

    p = add("set", cmd_set, "edit entity fields (field=value, field+=value)")
    p.add_argument("entity", help="leader | selN | partyN | entity index")
    p.add_argument("assignments", nargs="+", metavar="field=value")

    p = add("teleport", cmd_teleport, "place an entity on an arbitrary cell")
    p.add_argument("entity", help="leader | selN | partyN | entity index")
    p.add_argument("x", type=int)
    p.add_argument("y", type=int)
    p.add_argument("--facing", type=int, default=None, help="facing 0..7")
    p.add_argument("--no-halt", dest="halt", action="store_false",
                   help="keep the current order byte")

    p = add("items", cmd_items, "list an entity's items with resolved names")
    p.add_argument("entity", help="leader | selN | partyN | entity index")

    p = add("give", cmd_give, "create an item instance and attach it")
    p.add_argument("entity", help="leader | selN | partyN | entity index")
    p.add_argument("class_id", type=int, metavar="class")
    p.add_argument("subtype", type=int)
    p.add_argument("--qty", type=int, default=None, help="stack size (class 9/12)")
    p.add_argument("--condition", type=int, default=None, help="+0x01 byte (default: template)")
    p.add_argument("--modifier", type=int, default=None, help="+0x02 weapon modifier index")
    p.add_argument("--durability", type=float, default=None, help="current = max durability")
    p.add_argument("--poison", type=int, default=None, help="+0x0C poison/charge word")
    p.add_argument("--to", choices=("auto", "inventory", "equip", "additional", "ammo"),
                   default="auto", help="destination container")
    p.add_argument("--slot", type=int, default=None, help="destination slot index")

    p = add("remove", cmd_remove, "clear an item slot (or a whole container)")
    p.add_argument("entity", help="leader | selN | partyN | entity index")
    p.add_argument("where", choices=("equip", "inv", "add", "ammo", "all"))
    p.add_argument("slot", type=int, nargs="?", default=0)
    p.add_argument("--all", action="store_true",
                   help="clear every slot of the selected container")
    p.add_argument("--purge", action="store_true",
                   help="also mark the instances consumed (class 0xFF)")

    p = add("catalog", cmd_catalog, "list item prototypes (works without a game)",
            needs_proc=False)
    p.add_argument("--class", dest="class_id", type=int, default=None)
    p.add_argument("--subtype", type=int, default=None)
    p.add_argument("--search", default=None, help="substring of the CP866 name")

    add("discover", cmd_discover, "reveal all 56 world-map locations")

    p = add("travel", cmd_travel, "travel through the original world map")
    p.add_argument("location", type=int, help="location id 1..56")
    p.add_argument("--point", type=lambda s: tuple(int(v) for v in s.split(",")),
                   default=None, help="client x,y override")
    p.add_argument("--timeout", type=float, default=30.0)
    p.add_argument("--no-reveal", action="store_true",
                   help="do not reveal the map before clicking")
    p.add_argument("--no-orb", action="store_true",
                   help="do not add a travel orb when the leader has none")

    p = add("snapshot", cmd_snapshot, "write an exact KONUNG.SAn image")
    p.add_argument("path", type=Path)
    p.add_argument("--slot", type=int, default=None,
                   help="also install into the game's dir as KONUNG.SA<slot>")
    p.add_argument("--name", default=None, help="load-screen name for that slot")

    p = add("diff", cmd_diff, "diff two snapshot images (what did it write?)",
            needs_proc=False)
    p.add_argument("before", type=Path)
    p.add_argument("after", type=Path)
    p.add_argument("--blocks", default=None,
                   help=f"comma list of {','.join(DIFF_BLOCKS)}")
    p.add_argument("--entity", type=lambda s: [int(v, 0) for v in s.split(",")],
                   default=None,
                   help="only these entity indices, comma separated (entities block)")
    p.add_argument("--limit", type=int, default=20,
                   help="max printed records per block (default 20)")

    p = add("load-save", cmd_load_save, "install (and optionally load) a KONUNG.SAn image")
    p.add_argument("path", type=Path)
    p.add_argument("--slot", type=int, default=0)
    p.add_argument("--name", default=None)
    p.add_argument("--apply", action="store_true", help="drive the Load screen")
    p.add_argument("--timeout", type=float, default=60.0)

    p = add("new-game", cmd_new_game, "start a new game through the original UI")
    p.add_argument("--hero", type=int, default=0,
                   help="0 Dragomir, 1 Mikhail, 2 Wolf")
    p.add_argument("--class", dest="class_name", default="warrior",
                   choices=sorted(CLASS_ALIASES))
    p.add_argument("--keep", action="store_true",
                   help="do nothing when a game is already running")
    p.add_argument("--skip-intro", action="store_true", default=True,
                   help="skip the intro movie (default)")
    p.add_argument("--wait-intro", dest="skip_intro", action="store_false")
    p.add_argument("--timeout", type=float, default=90.0)

    p = add("selftest", cmd_selftest, "offline data-layer checks", needs_proc=False)
    return ap


def main(argv: list[str] | None = None) -> int:
    try:
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    except Exception:
        pass
    for stream in (sys.stdout, sys.stderr):
        try:
            stream.reconfigure(encoding="utf-8", errors="replace")
        except Exception:
            pass
    args = parser().parse_args(argv)
    proc = None
    try:
        if args.command == "selftest":
            return cmd_selftest(args)
        if getattr(args, "needs_proc", False):
            proc = GameProcess.attach(args.pid)
        return args.func(proc, args)
    except CheatError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 2
    except KeyboardInterrupt:
        print("interrupted", file=sys.stderr)
        return 130
    finally:
        if proc is not None:
            proc.close()


if __name__ == "__main__":
    raise SystemExit(main())
