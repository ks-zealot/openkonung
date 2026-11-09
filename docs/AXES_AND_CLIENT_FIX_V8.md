# v8: original 1999 axes + CLIENT.EXE profile

Two independent bugs were fixed.

## 1. MAPS placement axes

The 36-byte static placement record stores coordinates as:

- `+0x1E`: Y
- `+0x20`: X

The previous viewer treated them as X,Y and therefore forced the logical map into an incorrect 80x160 orientation. v8 restores the map grid to 160x80 and swaps those fields while parsing props.

Evidence: after swapping, original object IDs 244/245 align with the 1999 `Game.res` transition coordinates around the eight map-edge directions.

## 2. Original CLIENT.EXE support

The viewer used LAN_KONG.EXE virtual addresses against CLIENT.EXE. v8 has separate executable profiles:

- LAN_KONG.EXE 2003: DGROUP VA 0x470000, raw 0x54600
- CLIENT.EXE 1999 v1.03: DGROUP VA 0x440000, raw 0x29C00

The byte-identical equipment tables were located in CLIENT.EXE at:

- overlay palette table VA `0x44885C` (file offset `0x3245C`)
- item template table VA `0x44FEC0` (file offset `0x39AC0`)

When `--exe` is omitted the viewer now prefers `CLIENT.EXE`, then falls back to `LAN_KONG.EXE`.
