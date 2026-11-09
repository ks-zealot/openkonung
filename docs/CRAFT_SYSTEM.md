# Player alchemy / potion use

Target: `konung.res` (`sha256 d698c4f2…`). All addresses are VAs.

## 1. Input path

- Right-click an inventory strip item: `FUN_0042260C` right branch ->
  `FUN_0041DAB0(entity, 0, item)`.
- Held class-9 item + target class-9 item: `FUN_0041DAB0(entity, held, target)`
  -> `FUN_0041BB70(entity, held, target)`.
- The held item remains the product; the target is returned as an empty jar
  (`subtype 3`) or consumed (`class 0xFF`). `FUN_0042462C` inserts the target
  into inventory slot 0.

## 2. Recipe table

`0x450DF4`, 33 records × 6 bytes:

```text
{subtype_a, subtype_b, out_a, out_b, skill_gain, op}
```

Verified rows (class-9 subtypes):

| A | B | out A | out B | gain | op |
|---|---|---|---|---|---|
| 3 Пустая банка | 0 Ядовитое жало | 6 Яд | FF | 3 | 0 |
| 3 | 1 Земляной орех | 7 Масло | FF | 2 | 0 |
| 3 | 2 Белый корень | 5 Лечебный бальзам | FF | 0 | 0 |
| 5 Лечебный бальзам | 6 Яд | 8 Противоядие | 3 | 3 | 3 |
| 5 | 7 Масло | 9 Брага | 3 | 3 | 3 |
| 5 | 9 Брага | 10 Зелье | 3 | 0 | 3 |
| 6 | 7 | 11 Чистая слеза | 3 | 3 | 3 |
| 6 | 9 | 12 Эликсир Мудрости | 3 | 4 | 3 |
| 7 | 8 | 13 Живая Вода | 3 | 4 | 3 |

The full 33-row table is `inventory::RECIPES`. A missing row produces
«Непонятная смесь» (`subtype 4`) and consumes the target.

## 3. Quality

`op` uses the actor's healing skill (`Entity+0x48`):

```text
0: healing*0.01
1: quality + healing*0.01
2: quality + target_quality
3: min(quality, target_quality)
4: target_quality + healing*0.01
clamp 15.0
```

The actor's healing skill gains `skill_gain`, capped at 500.

## 4. Potion use (`FUN_0041DAB0`)

| subtype | effect |
|---|---|
| 4 Непонятная смесь | HP /= 2 (min 1) |
| 5 Лечебный бальзам | `min(1600, round(quality*160 + HP))` |
| 6 Яд | `periodic_damage += round(quality)` |
| 7 Масло | target item, quality >= 10, class 12: clear `+0x02` |
| 8 Противоядие | `max(0, round(quality*10 - base_armor))` |
| 9 Брага | timer 30·local; DEX −3·local; CHA/STR/END +local |
| 10 Зелье | timer 30·local; DEX/STR/END +local; HP heal |
| 11 Чистая слеза | timer 30·local; effect id `round(local*0.1)` |
| 12 Эликсир Мудрости | local >= 6; XP +(local−5)·100; timer 60·local; STR −5·local; HP −160·local (min 160 if CHA < 160) |
| 13 Живая Вода | HP 1600; revive |

The used item becomes subtype 3 (empty jar) in place. Temporary stats are saved
at `+0xBC..+0xC2` and restored when `+0xCE` reaches zero.

## 5. Remake

- `web/src/inventory.rs`: `RECIPES`, `craft`, `use_item`, `StatOp`,
  `apply_player_ops`/`apply_entity_ops`, `tick_*_temp`.
- `web/src/app/inventory.rs`: cursor lift/place, `try_craft_held_at`,
  `use_inventory_slot`, ammo stacking, ground drop, owner switching.
- `web/src/app/input.rs`: click/drag routing.
- `web/src/character_screen.rs`: owner-aware `CharacterSheet`, held preview.
