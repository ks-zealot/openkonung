# OpenKonung v13 playable prototype

This is the first interactive prototype built directly on the recovered original Konung 1.03 data model.

## Implemented

- starts on `Konung.1` using one of the three recovered `Game.N` start profiles;
- renders a human from the original `GRAPH.RES` human/equipment compositor;
- default visual player is human variant 16 (armed male fallback) until Dragomir's exact template is named;
- left click sets a movement target;
- idle uses human action 0;
- walking uses human action 1;
- movement direction selects one of all 8 original direction slots;
- exact `Game.N` transition records are live teleports;
- arriving at a transition source changes `Konung.N` and places the player at the recorded destination coordinates;
- `H` toggles the exact country graph.

## Run

From the original installed game directory:

```powershell
python .\START_PLAYABLE.py --base .
```

Choose another original start profile:

```powershell
python .\START_PLAYABLE.py --base . --profile 1
python .\START_PLAYABLE.py --base . --profile 2
```

Force another human variant:

```powershell
python .\START_PLAYABLE.py --base . --variant 16
```

## Controls

- LMB: walk
- MMB drag: pan camera
- mouse wheel: zoom
- F: fit current location
- H: toggle graph/current location
- RMB or Backspace: graph
- Esc: quit

## Known prototype limitations

- no pathfinding/collision yet; walking is straight-line;
- player is drawn over the cached static scene, so tree/building occlusion is not yet Y-sorted against the player;
- Dragomir's named variant has not yet been proven, so variant 16 is used as a visually valid male fallback;
- clicking a different node in graph view is a debug navigation shortcut and not original game behavior.

The important part is no longer synthetic: map geometry, human sprite frames, animation directions, and teleports all come from the original game data.
