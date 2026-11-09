# Playable prototype v13.1

Changes from v13:

- Default start profile is now `Game.2`, our current best candidate for Dragomir's authentic start.
- Camera starts centered on the player.
- Camera follows the player smoothly while walking.
- `C` toggles camera follow.
- Middle-mouse drag disables follow for manual inspection; press `C` to re-enable and snap back to the player.
- Mouse wheel zoom keeps the player centered while follow is enabled.
- Teleporting recenters the camera on the landing point.

Run:

```powershell
python .\START_PLAYABLE.py --base .
```
