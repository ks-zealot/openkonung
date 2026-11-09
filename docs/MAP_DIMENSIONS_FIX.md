# MAP coordinate correction (v8)

The earlier v4-v7 interpretation `MAP_W=80`, `MAP_H=160` was wrong.

Original `MAPS.RES` static placement records store:

- `+0x1E` = Y
- `+0x20` = X

The correct logical orientation used by the viewer is therefore `160 x 80`.

This is independently checked against original 1999 `Game.res`: after swapping the placement axes, boundary stones 244/245 line up with the eight transition-coordinate clusters near N/NE/E/SE/S/SW/W/NW.
