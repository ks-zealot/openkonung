# Digging / buried WorldObjects (`Лопата`)

The shovel (class-11 subtype 24) is **not** a use effect. A buried treasure is an
ordinary `Game.N` WorldObject record with a **negative money field**, and the
shovel is a *carried* requirement for collecting it.

## Data — the "dig table"

`Game.N +0x21900`, 500 × `0x40` (the same table the remake loads as
[`WorldObjectSource`](../web/src/world_objects.rs)):

| offset | type | meaning |
|---|---|---|
| `+0x00` / `+0x04` | i32 ×2 | activation timestamps (`FUN_0043ADF0` test) |
| `+0x08` / `+0x0A` | u16 ×2 | runtime-grid cell |
| `+0x0C` | u16 | ground sprite id (`FUN_00438CC8`) |
| `+0x0E` | u16 | source index (`0` ⇒ `WIN.FLM`) |
| **`+0x10`** | **i16** | **money — negative = buried** |
| `+0x12` | u16 | location id (`0` = permanently removed) |
| `+0x14..+0x3F` | u16 ×22 | item-instance ids |

There are **92 buried records** in each shipped `Game.0/1/2` (the coordinates
are identical across profiles; only the item-instance ids differ).

## Verified rules (`konung.res`, SHA-256 `d698c4f2…39810`)

1. **Active pool** — `FUN_0043ADF0` keeps every record whose location matches
   and whose `+0x00/+0x04` activation test passes. Money sign is irrelevant, so
   buried records *are* in the marker pool.
2. **Not drawn** — `FUN_0042532C` marker pass requires
   `-1 < (i16)marker[+0x10]`. Buried records stay invisible, with or without the
   torch.
3. **No cursor / no hover text** — `FUN_00428740` (`0x428A2B`) and the hover
   branch `0x423CD2` also require `money >= 0`; the hover message is
   `0x45C31C` «Что-то лежит на земле».
4. **Pickup gate** — `FUN_00421B50` on arrival (`Entity+0x03 & 0x40`):

   ```c
   if ((cell_flag_0x2000 != 0) &&
       (-1 < (i16)marker[+0x10] || FUN_00433A8C(0x19) != 0))
       FUN_004110EC(actor);
   ```

   `FUN_00433A8C(n)` scans the leader's **32 inventory slots** only
   (`Entity+0x72..+0xB1`) for a class `0x0B` item with `subtype == n-1` and
   returns `slot+1`. `0x19` ⇒ subtype `0x18` = **24 = Лопата**.
   Equipment and additional slots do not count.
5. **Money is `abs()`** — `FUN_0043EAA2` negates negatives, so a `-100` spot pays
   out `+100` (`FUN_004110EC` adds it before clearing the marker's money).
6. **Copper mirror (subtype 48)** — `FUN_00414DE4` case `0x30` negates `+0x10`
   for every record of the **current location** (source table and active
   markers), which makes the spot drawn / hoverable / collectable without the
   shovel. It is consumed.
7. **Shovel use (subtype 24, case `0x18`)** — sets interface mode `0`
   (`_DAT_0081D610`), base cursor `_DAT_0081D660 = 7`, plays sound 10, and does
   **not** consume the item. `FUN_0042260C` resets the cursor to `4` on the next
   left/right world click. No table lookup happens in the item case.
8. **Buried spots never respawn** — `FUN_0041093C` respawns only a sole class-9
   subtype-1/2 herb; every other emptied record gets `+0x12 = 0`.

## Remake implementation

| Behaviour | Code |
|---|---|
| buried records stay in `active_world_objects` | `app/world_items.rs::rebuild_world_objects` |
| skip drawing `money < 0` | `app/world_draw.rs` marker pass |
| ground cursor only for `money >= 0` | `app/cursor.rs` (`flag_2000`) |
| pickup gate `money >= 0 \|\| shovel in bag` | `app/world_items.rs::try_pickup_here`, `shovel_in_bag` |
| `abs(money)` payout | `app/world_items.rs::try_pickup_here` |
| shovel arms cursor frame 7, not consumed | `app/class11.rs` case 24, `PlayableApp::dig_armed` |
| mirror flips current-location money | `app/class11.rs::class11_mirror` |

Tests: `app::tests::world_items::e1_*`, `app::tests::class11::e1_*`,
`app::tests::class11::c4_mirror_reactivates_negative_ground_records`.

## Game.1 buried-spot table

| loc | cell | money | contents |
|---|---|---|---|
| 1 | (95,13) | -100 | Медное зеркало колдуна |
| 1 | (157,44) | -100 | шт. Cеребряные стрелы ×3 |
| 3 | (98,20) | -100 | Медное зеркало колдуна |
| 3 | (112,93) | -100 | Шлем клепаный |
| 4 | (69,30) | -100 | Элексир Мудрости |
| 4 | (90,10) | -100 | шт. Железные стрелы ×3 |
| 5 | (26,20) | -100 | Магический шар волхва Старого лагеря |
| 5 | (38,24) | -100 | Элексир Мудрости |
| 5 | (62,24) | -50 | Ожерелье |
| 7 | (24,72) | -100 | Богатырский шлем |
| 10 | (69,102) | -100 | шт. Cеребряные стрелы ×2 |
| 10 | (100,65) | -100 | Шлем датский |
| 11 | (31,42) | -100 | Слова Воскрешения |
| 11 | (91,76) | -100 | шт. Cеребряные стрелы ×3 |
| 12 | (82,66) | -100 | Живая Вода |
| 12 | (88,71) | -100 | Факел |
| 12 | (96,69) | -100 | Браслет |
| 14 | (130,35) | -100 | Чертежи древних пирамид |
| 14 | (160,29) | -100 | Факел |
| 14 | (192,50) | -100 | Элексир Мудрости |
| **15** | **(50,49)** | **-100** | **Лезвие древнего меча** |
| 15 | (74,82) | -100 | Живая Вода |
| 15 | (121,52) | -30 | Кольцо |
| 16 | (102,52) | -100 | Паутина странного паука |
| 16 | (120,50) | -100 | Элептус |
| 16 | (132,38) | -200 | Ожерелье |
| 17 | (64,42) | -100 | Шлем яйцевидный |
| 17 | (88,77) | -100 | Браслет |
| 19 | (34,69) | -100 | Чаша воеводы |
| 19 | (156,13) | -100 | Магический шар волхва Черного бора |
| 20 | (93,17) | -100 | Медное зеркало колдуна |
| 20 | (162,16) | -100 | Слова Воскрешения |
| 23 | (176,33) | -100 | Богатырские доспехи |
| 24 | (37,21) | -100 | Длинный лук |
| 24 | (60,48) | -100 | Браслет |
| 24 | (95,57) | -100 | Детские игрушки |
| 25 | (35,108) | -100 | Доспех кожано-металлический |
| 25 | (77,58) | -100 | Спата |
| 25 | (110,73) | -100 | Топор |
| 25 | (144,118) | -100 | Трактат о торговле |
| 25 | (216,84) | -100 | Элексир Мудрости |
| 28 | (51,28) | -100 | Браслет |
| 28 | (151,39) | -100 | Соколиная лапка |
| 28 | (196,35) | -100 | Слова Воскрешения |
| 30 | (45,5) | -100 | Элексир Мудрости |
| 30 | (89,104) | -100 | Медное зеркало колдуна |
| 33 | (12,5) | -100 | Ожерелье |
| 33 | (36,14) | -100 | Короткий лук |
| 33 | (59,4) | -100 | Щит строевой |
| 35 | (143,50) | -100 | Ожерелье |
| 35 | (166,91) | -100 | Медное зеркало колдуна |
| 36 | (22,72) | -100 | Браслет |
| 36 | (57,113) | -100 | Живая Вода |
| 36 | (111,30) | -100 | Слова Воскрешения |
| 37 | (16,77) | -200 | Ожерелье |
| 37 | (94,50) | -100 | шт. Cеребряные стрелы ×2 |
| 38 | (92,39) | -100 | Шлем катафрактный |
| 38 | (122,14) | -100 | Кольцо |
| 38 | (161,61) | -100 | Составной лук |
| 39 | (20,100) | -100 | шт. Cеребряные стрелы ×2 |
| 39 | (95,21) | -100 | Медное зеркало колдуна |
| 39 | (154,47) | -100 | Щит длинный |
| 40 | (56,60) | -100 | Свисток |
| 40 | (84,26) | -100 | Скутум |
| 42 | (10,41) | -100 | Пехотный лук |
| 42 | (18,7) | -100 | Шлем клепаный |
| 42 | (60,16) | -100 | Магический шар волхва Родников |
| 42 | (65,31) | -300 | Кольцо |
| 43 | (12,17) | -100 | Щит овальный |
| 43 | (37,14) | -100 | Клееный лук |
| 43 | (44,29) | -100 | Шишак |
| 43 | (59,28) | -100 | Ожерелье |
| 44 | (48,41) | -100 | Короткий лук |
| 44 | (59,22) | -100 | Медное зеркало колдуна |
| 44 | (87,15) | -100 | Живая Вода |
| 44 | (108,40) | -100 | Меч |
| 45 | (13,39) | -100 | Шлем гребневый |
| 45 | (85,5) | -100 | Браслет |
| 45 | (87,22) | -100 | Чаша воеводы |
| 45 | (108,40) | -100 | Щит миндалевидный |
| 48 | (61,7) | -100 | Живая Вода |
| 48 | (75,12) | -100 | Медное зеркало колдуна |
| 48 | (76,2) | -100 | Шлем кольчужный |
| 48 | (107,12) | -100 | Длинный меч |
| 49 | (18,8) | -100 | Богатырский топор |
| 49 | (43,15) | -100 | Палица |
| 49 | (56,21) | -100 | Живая Вода |
| 49 | (79,19) | -100 | Медное зеркало колдуна |
| 50 | (38,6) | -100 | Дупликатор |
| 50 | (68,16) | -100 | Кольчуга |
| 50 | (95,7) | -100 | Шар магического прыжка ×3 |
| 50 | (118,18) | -100 | Живая Вода |

`Рукоять древнего меча` (subtype 22) is **not** buried: it is an ordinary
visible pile at location 36, cell (34,82), money 0. Combine the two parts with
the duplicator/craft slot to get the sword (class 2 subtype 0x1B).
