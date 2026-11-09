# OpenKonung playable prototype v14

## Native 1999-style camera

Gameplay now renders to a fixed **800x600 virtual framebuffer** at 1:1 world-pixel scale. The completed frame is then scaled to the actual host window while preserving 4:3, so widescreen displays get pillarboxing instead of showing vastly more of the map than the original game.

The shipped 1999 data contains parallel `*640R.RES` and `*800R.RES` UI resources; v14 uses the 800x600 presentation as the default reconstruction target.

## Live portals anchored to the stones

`Game.N` remains authoritative for destination map and landing coordinates. Its source coordinates are *not* drawn directly because they are not in the same coordinate system as the rendered `Konung.N` placement anchors.

Instead v14 binds each outgoing exact transition to a detected visual gate built from `OBJECTS 244/245`, and places the active trigger at the midpoint of the physical stone pedestals. A small double ring is drawn there as a temporary stand-in for the original animated portal glyph until the exact effect resource is identified.

Binding is deterministic and prefers matching edge/gate side, then stable placement order. The destination itself is never guessed.

## Controls

- LMB: walk
- C: camera follow on/off
- MMB drag: free camera
- H: graph
- RMB / Backspace: graph
- Esc: quit

Gameplay zoom is intentionally fixed. The old mouse-wheel zoom still works in graph view only.
