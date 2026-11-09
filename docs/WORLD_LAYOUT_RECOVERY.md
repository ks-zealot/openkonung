# World layout recovery notes

Status: **provisional reconstruction**.

The original `world_layout.json` in this prototype was a neutral 5x4 row-major placeholder.
This update replaces it with a data-driven stitched arrangement recovered from the shipped resource files.

## Current 5x4 arrangement

Numbers below are **1-based map numbers** for readability.

```text
Row 0:   5  13 15 18  8
Row 1:  19  12  3 14  1
Row 2:  17  11 10  2 20
Row 3:   7   6  4 16  9
```

Equivalent `world_layout.json` entries are zero-based map ids.

## Why this layout

I used a recovery heuristic instead of pure guesswork:

1. Read each map's terrain grid from `MAPS.RES`.
2. Compare east-west and north-south border strips.
3. Weight rare terrain ids more strongly than common grass tiles.
4. Search many 5x4 permutations for a low seam-cost arrangement.
5. Keep a result that also groups similar biomes into believable regions.

That gives a stitched canvas that is materially better than the old placeholder.

## Confidence / caveat

This should be treated as the **best recovered seamless canvas so far**, not as a final archival truth.
The LAN/server topology may still have used teleports, wrap rules, or a non-rectangular adjacency graph that is not fully exposed yet.
If we later decode an explicit transition table from the executable or a hidden config block, we should replace this layout with the canonical one.

## Files

- `world_layout.json` — updated provisional reconstruction.
- `world_layout_original_rowmajor.json` — backup of the original placeholder layout.
