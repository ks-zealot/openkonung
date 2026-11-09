# v10: automatic Forest Country reconstruction

Run in the extracted original-install directory:

```powershell
python .\konung_country_graph.py --base .
```

The program scans every `Konung.N`, identifies which of the 20 `MAPS.RES` base templates it most closely derives from using rarity-weighted terrain fingerprints, reads the original 20-map directional topology from `Game.res` at `0x41700`, and pairs active local teleport gates only when reciprocal constraints support the match.

It writes `country_graph_auto.json` next to the game files and opens the reconstructed atlas. The layout follows recovered teleport directions instead of file-number order.

Important: v10 deliberately labels the links as reconstructed rather than pretending the destination field has already been proven. `high` links are mutual-best reciprocal matches; ambiguous exits remain unresolved. This gives us a usable Forest Country graph now while keeping the forensic distinction clear.
