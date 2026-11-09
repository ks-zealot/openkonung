# v12 local Konung.N geometry fix

The previous local renderer reused the MAPS.RES 160x80 geometry for Konung.N.
That produced extremely wide scenes and exposed off-map props over a black void.

Recovered local geometry:

- terrain: 12800 u16 = 80 columns x 160 rows
- terrain anchor: x = 58*column - 29*(row&1), y = 16*row
- Konung.N placement +0x1E = X column, +0x20 = Y row
- Game.N transition X uses half-column units: screen_x = origin + 29*x
- Game.N transition Y: screen_y = 16*y
- local render canvas is bounded to terrain; out-of-map placement records no longer enlarge it

This is separate from the MAPS.RES world-chunk geometry.

Use `python START_FOREST_COUNTRY.py --base .` or `python konung_exact_country.py --base .`.
Do not use `konung_location_graph.py` for the exact graph; it is retained only as an older diagnostic tool.
