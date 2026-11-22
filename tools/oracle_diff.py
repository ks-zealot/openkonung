"""Compare the original and remake halves of the screenshot harness.

    python web/tools/oracle_diff.py [--dir web/tools/oracle]

Reads the ``original-<frame>-<cfg>.png`` / ``remake-<frame>-<cfg>.png`` pairs
produced by ``oracle_capture.py`` and ``oracle_frames.mjs`` and reports:

* the selection-ellipse position of each shot (green stroke, exact palette
  colour ``(0, 251, 0)``), used to align the two cameras: the remake centres
  the *ground projection* while the original centres the sprite anchor
  (``Entity+0xE4/+0xEC``), an open camera-parity gap;
* per-pair metrics over the world viewport after that alignment;
* the per-engine effect of each render toggle (shadows / day-night /
  light-effects off against the ``all`` baseline), which is independent of the
  camera gap because both shots of an engine share the same camera.

Only the world viewport (x >= 140, y < 545) is compared: the HUD and the
game-drawn cursor are not part of this wave.
"""
from __future__ import annotations

import argparse
import struct
import sys
import zlib
from pathlib import Path

try:
    from PIL import Image
except ImportError:  # pragma: no cover - the repo venv ships Pillow
    Image = None

FRAMES = (0, 6000, 12000, 18000)
CONFIGS = ("all", "noshadow", "dayonly", "nolight")
RING_RGB = (0, 251, 0)
VIEW_X = 140
VIEW_Y = 0
VIEW_W = 660
VIEW_H = 545


def load(path: Path):
    im = Image.open(path).convert("RGB")
    return im, im.load(), im.size


def ring_center(px, w: int, h: int):
    xs = []
    ys = []
    for y in range(VIEW_Y, VIEW_H):
        for x in range(VIEW_X, VIEW_X + VIEW_W):
            if px[x, y] == RING_RGB:
                xs.append(x)
                ys.append(y)
    if not xs:
        return None
    return (min(xs) + max(xs)) / 2.0, (min(ys) + max(ys)) / 2.0


def compare(a, apx, b, bpx, ax: int, ay: int):
    """Mean absolute difference and closeness over the world viewport.

    ``a`` is sampled at ``(x + ax, y + ay)``; ``b`` at ``(x, y)``.
    """
    total = 0
    close = 0
    exact = 0
    n = 0
    aw, ah = a.size
    for y in range(VIEW_Y, VIEW_H):
        by = y
        ay2 = y + ay
        for x in range(VIEW_X, VIEW_X + VIEW_W):
            bx = x
            ax2 = x + ax
            if not (0 <= ay2 < ah and 0 <= ax2 < aw):
                continue
            ca = apx[ax2, ay2]
            cb = bpx[bx, by]
            d = abs(ca[0] - cb[0]) + abs(ca[1] - cb[1]) + abs(ca[2] - cb[2])
            total += d
            if max(abs(ca[i] - cb[i]) for i in range(3)) <= 8:
                close += 1
            if ca == cb:
                exact += 1
            n += 1
    if n == 0:
        return None
    return {
        "n": n,
        "mean": total / (3.0 * n),
        "close": close / n,
        "exact": exact / n,
    }


def effect(a, apx, b, bpx):
    """Per-engine toggle effect (same camera in both shots)."""
    total = 0
    changed = 0
    n = 0
    for y in range(VIEW_Y, VIEW_H):
        for x in range(VIEW_X, VIEW_X + VIEW_W):
            ca = apx[x, y]
            cb = bpx[x, y]
            d = abs(ca[0] - cb[0]) + abs(ca[1] - cb[1]) + abs(ca[2] - cb[2])
            total += d
            if d > 24:
                changed += 1
            n += 1
    return {"mean": total / (3.0 * n), "changed": changed / n}


def write_png(path: Path, w: int, h: int, rgba: bytes) -> None:
    def chunk(tag: bytes, data: bytes) -> bytes:
        return (struct.pack(">I", len(data)) + tag + data
                + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF))

    stride = w * 4
    raw = b"".join(b"\0" + rgba[y * stride:(y + 1) * stride] for y in range(h))
    path.write_bytes(b"\x89PNG\r\n\x1a\n"
                     + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
                     + chunk(b"IDAT", zlib.compress(raw, 6))
                     + chunk(b"IEND", b""))


def side_by_side(out: Path, a, apx, bpx, ax: int, ay: int) -> None:
    """Write original | remake | abs-diff over the world viewport."""
    w, h = VIEW_W, VIEW_H
    aw, ah = a.size
    body = bytearray(w * 3 * h * 4)
    for y in range(h):
        base = y * w * 3
        for x in range(w):
            gx = x + VIEW_X
            ca = apx[gx + ax, y + ay] if 0 <= y + ay < ah and 0 <= gx + ax < aw else (0, 0, 0)
            cb = bpx[gx, y]
            d = tuple(abs(ca[i] - cb[i]) for i in range(3))
            for panel, rgb in enumerate((ca, cb, d)):
                o = (base + panel * w + x) * 4
                body[o:o + 4] = bytes(rgb) + b"\xff"
    write_png(out, w * 3, h, bytes(body))


def main(argv: list[str] | None = None) -> int:
    try:
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    except Exception:
        pass
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--dir", type=Path,
                    default=Path(__file__).resolve().parent / "oracle")
    args = ap.parse_args(argv)
    if Image is None:
        raise SystemExit("Pillow is required (run with web/../.venv python)")

    print(f"{'pair':28s} {'ring o':>12s} {'ring r':>12s} {'shift':>10s} "
          f"{'mean|d|':>8s} {'close%':>7s} {'exact%':>7s}")
    for frame in FRAMES:
        # The alignment comes from the no-shadow shot: the shadow darken clips
        # the selection-ring stroke asymmetrically, so the shadowed captures'
        # green bounding box is not a reliable camera reference. The camera is
        # identical across the configs (same forced state).
        ref_o = args.dir / f"original-{frame}-noshadow.png"
        ref_r = args.dir / f"remake-{frame}-noshadow.png"
        frame_shift = None
        if ref_o.exists() and ref_r.exists():
            ro, rpx, _ = load(ref_o)
            rr, rrpx, _ = load(ref_r)
            co = ring_center(rpx, *ro.size)
            cr = ring_center(rrpx, *rr.size)
            if co is not None and cr is not None:
                frame_shift = (round(cr[0] - co[0]), round(cr[1] - co[1]))
        for cfg in CONFIGS:
            po = args.dir / f"original-{frame}-{cfg}.png"
            pr = args.dir / f"remake-{frame}-{cfg}.png"
            if not po.exists() or not pr.exists():
                print(f"{frame}-{cfg}: missing image")
                continue
            ao, apx, _ = load(po)
            ar, rpx, _ = load(pr)
            co = ring_center(apx, *ao.size)
            cr = ring_center(rpx, *ar.size)
            if co is None or cr is None:
                print(f"{frame}-{cfg}: ring not found (o={co} r={cr})")
                continue
            ax, ay = frame_shift if frame_shift else (
                round(cr[0] - co[0]), round(cr[1] - co[1])
            )
            m = compare(ao, apx, ar, rpx, ax, ay)
            print(f"{frame:5d}-{cfg:10s} ({co[0]:5.1f},{co[1]:5.1f}) "
                  f"({cr[0]:5.1f},{cr[1]:5.1f}) ({ax:+3d},{ay:+3d}) "
                  f"{m['mean']:8.2f} {m['close']*100:6.1f}% {m['exact']*100:6.1f}%")
            if cfg == "all":
                side_by_side(args.dir / f"diff-{frame}-{cfg}.png", ao, apx, rpx, ax, ay)

    print("\ntoggle effect (world viewport, mean |d| / changed pixels)")
    for frame in FRAMES:
        print(f"  frame {frame}")
        for cfg in ("noshadow", "dayonly", "nolight"):
            for prefix in ("original", "remake"):
                base = args.dir / f"{prefix}-{frame}-all.png"
                var = args.dir / f"{prefix}-{frame}-{cfg}.png"
                if not base.exists() or not var.exists():
                    continue
                ia, ipx, _ = load(base)
                ib, jpx, _ = load(var)
                e = effect(ia, ipx, ib, jpx)
                print(f"    {prefix:8s} {cfg:9s} mean {e['mean']:6.2f}  "
                      f"changed {e['changed']*100:5.1f}%")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
