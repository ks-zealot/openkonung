# v9: local location atlas and teleport graph

New entry point: `konung_location_graph.py`.

It scans every numeric `Konung.N` in the original installed game directory, renders it through the existing GRAPH/OBJECTS decoder, detects OBJECTS 244/245 teleport stones, groups nearby stones into local gates, classifies each gate by side, and writes `location_graph_report.json`.

The graph intentionally does **not** invent destinations. The local-map destination field has not yet been proven. Detected gates are shown as pink dangling ports. If `location_links.json` is present, proven/manual links are drawn between location nodes.

Run from the unpacked original game directory:

    python konung_location_graph.py --base .

Controls: click a node to open it, H returns to graph, mouse drag pans, mouse wheel zooms, R rebuilds render caches.

Important: Konung.N placement anchors can extend beyond the old MAPS.RES 160x80 terrain domain. v9 extends the render canvas instead of clipping those props. Terrain scan order remains the currently verified 160x80 interpretation until the local-map coordinate model is closed completely.


## v9.1 crash fix
Fixed detail-view gate projection: grouped gate centroids are fractional, while the legacy staggered-grid transform requires integer row parity. v9.1 projects the original integer teleport-marker cells first and averages their screen coordinates.
