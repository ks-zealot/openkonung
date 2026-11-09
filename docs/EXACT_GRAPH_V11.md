# v11: exact Forest Country graph

The previous v10 fingerprint reconstruction is retired for topology.

The original installed `konung.res` is itself a PE32 game executable (timestamp 1999-10-08), not a passive resource file. Its loader constructs `GAME.<n>`, `LIGHTS.<n>` and `KONUNG.<n>` filenames directly.

For `GAME.0`, `GAME.1`, `GAME.2` the loader reads six sequential blocks. Their sizes sum exactly to 709,822 bytes, the full file size:

```
0x00000  0x20000
0x20000  0x01900
0x21900  0x07D00
0x29600  0x0109A   <-- 250 x 17-byte transition records
0x2A69A  0x7D000
0xA769A  0x05E24
end      0xAD4BE
```

The 17-byte table contains direct local-location links. `+3` is source `Konung.N`, `+4` is destination `Konung.N`; source and destination cell coordinates follow as little-endian u16 values.

So v11 no longer guesses topology from terrain fingerprints. It reads the game's own graph.

Run:

```
python .\konung_exact_country.py --base .
```

Profiles:

```
--profile 0
--profile 1
--profile 2
--profile common
--profile union   (default)
```

Outputs:

- `forest_country_exact_graph.json`
- `forest_country_edges.csv`

The graphical placement of map cards is still an embedding of the exact graph, not yet a proven seamless metric projection. Transition edges and transition coordinates are exact data from GAME.N.
