# Playable v15.2 — original GAME.RES human fallback fix

The palette bug was not a palette decoder bug. The playable prototype forced GAME variant 16 as the fallback player.

That assumption came from the later LAN GAME.RES. In the original 1999 Game.res, variants 0..47 are placeholder/startup records: base human 0, palette 33, no equipment. Variant 16 is therefore the wrong visual template and produces the pale/purple figure.

The fallback selector now scans the actually loaded Game.res and chooses the first equipped, renderable human with a valid palette. On original v1.03 this selects variant 48. Its native record is used unchanged for base graphics, palette and equipment.

Direction mapping and the v15 portal placement are unchanged.

This is still a fallback NPC, not yet proven to be Dragomir. The next canonical step is to recover the actual player entity/template from Game.2.
