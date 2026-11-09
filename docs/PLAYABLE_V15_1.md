# Playable v15.1 — direction + palette fix

- Corrected human direction slot order from decoded frames: `0=W, 1=NW, 2=N, 3=NE, 4=E, 5=SE, 6=S, 7=SW`.
- Walking down/south now selects slot 6. Walking up/north selects slot 2.
- Removed the invalid `Game.N` palette override introduced in v15. The previous `START_STATE_OFF + 0x41` read was not a proven human entity palette field.
- Player now uses the palette carried by the selected human variant itself. Fallback variant 16 uses palette 36 in the recovered GAME.RES data.
- Portal placement/activation from v15 is unchanged.
