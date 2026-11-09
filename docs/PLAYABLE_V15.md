# OpenKonung playable v15

Changes:

- Player base palette is now read from the selected original `Game.N` start-state record instead of being forced by fallback `GAME.RES` variant 16.
- Profile 2 currently yields palette id 32 from the original start state.
- Human animation still uses the fallback human body/equipment template until the exact named Dragomir template is proven.
- Live teleport anchors remain bound to the real 244/245 stone pairs.
- The debug green ring is replaced with a flat metallic plate stand-in between the stones, matching the remembered original presentation more closely. The exact original plate sprite/effect is not yet identified.
- Gameplay camera remains fixed at native 800x600 and follows the player.

Run:

```powershell
python .\START_PLAYABLE.py --base .
```
