# World layout recovered from teleport transitions

This is no longer a terrain-edge heuristic.

`GAME.RES` contains a transition block at **0x41700**:

```text
250 records × 17 bytes = 0x109A bytes
```

The first **160 records** are exactly the overworld map transitions:

```text
20 maps × 8 compass directions
```

Each canonical record contains:

```text
+0x03  source map id (1..20)
+0x04  destination map id (1..20)
+0x05  source X (u16)
+0x07  source Y (u16)
+0x09  destination X (u16)
+0x0B  destination Y (u16)
+0x0D  destination X copy / related target coordinate
+0x0F  destination Y copy / related target coordinate
```

The source coordinates fall into eight stable gate positions, so every record can be assigned to N/NE/E/SE/S/SW/W/NW without guessing.

All 160 links are reciprocal. Solving their adjacency constraints gives one and only one rectangular torus:

```text
4 × 5 maps
```

Using map 1 as the upper-left cut:

```text
 1  11   6   3
15  10   5   2
14   9   4  18
17  13   8  20
16  12   7  19
```

The original transition network wraps both horizontally and vertically. In other words, the LAN world is topologically a **4×5 torus** with 8-neighbour connectivity.

For the seamless viewer we use the table above as one finite cut of that torus. Objects **244/245** are the boundary teleport stones and are hidden by default in the fast viewer; press **F2** to show them for archaeology/debugging.

`world_topology_teleports.json` preserves every decoded transition record. `recover_world_layout.py` regenerates the layout directly from an original `GAME.RES` and verifies reciprocity and the unique 4×5 torus solution.
