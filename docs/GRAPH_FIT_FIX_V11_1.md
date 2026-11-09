# v11.1 graph fit fix

The exact graph was being generated correctly but opened with `zoom=1` and `pan=(0,0)`.
The recovered graph spans roughly 17.5k x 10k pixels at that scale, and no node happened to intersect the initial top-left viewport, so only the HUD was visible.

v11.1 computes graph bounds at startup, chooses a fit-to-window zoom, and centers the graph automatically.
No graph data or transition decoding changed.
