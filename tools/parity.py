#!/usr/bin/env python3
"""Reference parity dump using the original Python decoders.

Prints the same stable hashes as `cargo run --example dump` so the Rust port can
be verified byte-for-byte against the source of truth.

Run from the original install dir:
    python web/tools/parity.py [base] [profile] [variant]
"""
from __future__ import annotations

import os
import sys
from pathlib import Path

os.environ.setdefault("SDL_VIDEODRIVER", "dummy")

HERE = Path(__file__).resolve().parent
BASE = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else HERE.parent.parent
PROFILE = int(sys.argv[2]) if len(sys.argv) > 2 else 2
VARIANT = int(sys.argv[3]) if len(sys.argv) > 3 else -1

sys.path.insert(0, str(BASE))
import konung_exact_country as ec  # noqa: E402
from PIL import Image  # noqa: E402

mod = ec.mod
lg = mod  # konung_location_graph module


def fnv(data: bytes) -> int:
    h = 0xCBF29CE484222325
    for b in data:
        h ^= b
        h = (h * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return h


def resolve(name, explicit=None):
    return lg.resolve(BASE, explicit, name)


exe = None
for n in ("CLIENT.EXE", "LAN_KONG.EXE"):
    try:
        exe = resolve(n)
        break
    except FileNotFoundError:
        pass
res = lg.KonungResources(
    resolve("Game.res"), resolve("GRAPH.RES"), resolve("Objects.res"), resolve("Maps.res"), exe
)

variant = VARIANT if VARIANT >= 0 else None
# emulate Rust find_fallback_human
if variant is None:
    vr = None
    for vid in range(lg.dec.GAME_VARIANT_COUNT):
        v = res.variant(vid)
        if len(v) < 0x91 or (v[4] & 0x40):
            continue
        if v[0x3F] >= len(res.human_base_positions):
            continue
        if not any(lg.u16(v, 0x45 + s * 2) for s in range(6)):
            continue
        po = lg.u32(v, 0x41)
        if po % 512 or not (0 <= po // 512 < 256):
            continue
        variant = vid
        break

print(f"profile={PROFILE}")
print(f"variant={variant}")

p0 = bytes(c for rgb in res.palettes[0] for c in rgb)
p36 = bytes(c for rgb in res.palettes[36] for c in rgb)
print(f"palette0={fnv(p0):016x}")
print(f"palette36={fnv(p36):016x}")

for tid in (0, 123):
    im = res.terrain_tile(tid)
    raw = im.tobytes("raw", "RGBA")
    print(f"tile{tid}={fnv(raw):016x} {im.width}x{im.height}")

im, gx, gy = res.prop_sprite(30, 0, 0)
print(f"prop30={fnv(im.tobytes('raw','RGBA')):016x} {im.width}x{im.height} off={gx},{gy}")

h, hx, hy, hc = res.human_frame_state(variant, 0, 0, 0)
print(f"human_a0_d0_f0={fnv(h.tobytes('raw','RGBA')):016x} {h.width}x{h.height} off={hx},{hy} count={hc}")

# locations -- count only (rendering compared separately below)
import re  # noqa: E402

kon = [p for p in BASE.iterdir() if p.is_file() and re.match(r"^konung\.\d+$", p.name, re.I)]
print(f"loc_count={len(kon)}")

profiles = ec.discover_game_profiles(BASE)
locs = lg.load_locations(BASE)
edges = ec.build_exact_edges(PROFILE, profiles, locs)
pairs, one_way, self_edges = ec.reciprocal_pairs(edges)
print(f"edges={len(edges)} pairs={len(pairs)} one_way={len(one_way)} self={len(self_edges)}")

layout = ec.spring_layout(locs, pairs, one_way)
lid0 = sorted(layout)[0]
x, y = layout[lid0]
print(f"spring0 id={lid0} x={x:.4f} y={y:.4f}")


# Reproduce Renderer.render_location with PIL to compare canonical RGBA bytes.
# Layer order matches Rust `render::render_location_surface`: terrain low then
# high, then GRAPH placements (identity anchors), then props sorted by projected
# baseline (grid_x, grid_y).
def render_location_pil(loc):
    surf = Image.new("RGBA", (lg.LOCAL_PIXEL_W, lg.LOCAL_PIXEL_H), (8, 12, 9, 255))
    cells = memoryview(loc.data)[lg.TERRAIN_OFF: lg.TERRAIN_OFF + lg.TERRAIN_SIZE]
    for cy in range(lg.LOCAL_TERRAIN_H):
        for cx in range(lg.LOCAL_TERRAIN_W):
            o = (cy * lg.LOCAL_TERRAIN_W + cx) * 2
            rec = cells[o] | (cells[o + 1] << 8)
            low, high = lg.terrain_layers(rec)
            x, y = lg.local_terrain_to_screen(cx, cy)
            for r in (low, high):
                if r is None:
                    continue
                ts = res.terrain_tile(r)
                if ts:
                    surf.alpha_composite(ts, (x, y))
    gp = memoryview(loc.data)[lg.META_OFF: lg.META_OFF + 1000 * 0x10]
    for i in range(1000):
        o = i * 0x10
        t = int.from_bytes(gp[o:o + 4], "little")
        if t == 0xFFFFFFFF:
            continue
        gx = int.from_bytes(gp[o + 12:o + 14], "little")
        gy = int.from_bytes(gp[o + 14:o + 16], "little")
        ts = res.terrain_tile(t & 0x1FF)
        if ts:
            lx, ly = lg.graph_anchor_to_local_px(gx, gy)
            surf.alpha_composite(ts, (lx, ly))
    for p in sorted(loc.props, key=lambda q: (q.x, q.y, q.ri)):
        try:
            ps, gx, gy = res.prop_sprite(p.object_id, 0, p.palette)
        except Exception:
            continue
        x, y = lg.prop_to_local_px(p.x, p.y)
        surf.alpha_composite(ps, (x + gx, y + gy))
    return surf


l1 = next(l for l in locs if l.loc_id == 1)
surf1 = render_location_pil(l1)
print(f"loc1={fnv(surf1.tobytes('raw','RGBA')):016x} {surf1.width}x{surf1.height}")
