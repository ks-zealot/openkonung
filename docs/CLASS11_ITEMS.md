# Class 11 — special / quest / usable items

`ItemInstance +0x00 = 0x0B` is the game's **special / quest / usable** item
class: maps, books, magical orbs, keys, quest artifacts, the shovel, etc. It is
unrelated to character classes.

## Dispatcher

`FUN_00414DE4(actor, target_item, used_item)` (`0x414DE4`) switches on the used
instance's subtype (`+0x03`) only when `*(u8*)used == 0x0B`. It is called from:

- `FUN_0042033C` — use a held item in hand (`target = NULL`);
- `FUN_0042260C` — held item applied to the item under the cursor
  (`target = _DAT_0081D670`, used = `_DAT_0081D400`).

Class-9 potions use `FUN_0041DAB0` instead.

Each case may set `*used = 0xFF` (consume) and/or apply an effect. Several
cases are reusable (no class write): 7, 14–21, 24, 31, 35/37, 44/52/53.
Subtype 33 («Кукла») **does** consume; subtype 11 («Сын Луны») has no case at
all and is never consumed.

## Subtype catalogue (shipped names)

Names come from the class 10–11 table at `0x46091C` (stride 128, CP866).

| # | Name | Effect | Remake |
|---|---|---|---|
| 0 | Потерянная карта | Reveal one random undiscovered location | **C1 done** (`use_map_item`) |
| 1 | Труды Гиппократа | Healing +100 (cap 500) + message | **C2 done** |
| 2 | Чертежи древних пирамид | Building +5 + message | **C2 done** |
| 3 | Свиток кузнеца | Smithing +4 + message | **C2 done** |
| 4 | Трактат о торговле | Trade +10 (cap 100) + message | **C2 done** |
| 5 | Ягода Третий Глаз | Identification +100 (cap 100) + message | **C2 done** |
| 6 | Слова Воскрешения | Revive every dead actor in the location (`FUN_00414ABC`) | **C4 done** |
| 7 | Дупликатор | Copy the target 16-byte item instance onto the used item | **C3 done** |
| 8 | Соколиная лапка | Strength current +3 | **C2 done** |
| 9 | Свиток ведуна | Clear instance `+0x0E & 1` (unidentified) on all owned items; leader identification +1 each (cap 100) | **C2 done** |
| 10 | Яблоко познания | +200 XP | **C2 done** |
| 11 | Сын Луны | **Passive:** class-11 template weight `0xD8F0` = `i16` −10000 (−10.0 kg); `FUN_0041B7E4` sums it signed. No switch case; never consumed. | **done (D1)** |
| 12 | Вино | Temporary buff (potion-style; `n = min(dex_base/3, 5)`, timer `30n`) | **C2 done** |
| 13 | Великий магнит | Transfer other groups' money to leader and items to the user | **C4 done** |
| 14 | Шар волхва Бережков | Teleport 28,184,18,1 | **C4 done** |
| 15 | Шар волхва Старого лагеря | Teleport 5,60,68,2 | **C4 done** |
| 16 | Шар волхва Византийского лагеря | Teleport 10,147,66,1 | **C4 done** |
| 17 | Шар волхва Чёрного бора | Teleport 20,49,41,2 | **C4 done** |
| 18 | Шар волхва Родников | Teleport 3,141,54,1 | **C4 done** |
| 19 | Шар волхва Лесного лагеря | Teleport 40,42,31,1 | **C4 done** |
| 20 | Шар магического прыжка | Open world map (mode 5) | **C4 done** |
| 21 | Шар Подземного города | Teleport 43,18,24,2 | **C4 done** |
| 22 | Рукоять древнего меча | Combine with subtype 23 → class 2 subtype 0x1B, +30 XP | **C3 done** |
| 23 | Лезвие древнего меча | Combine with subtype 22 → class 2 subtype 0x1B, +30 XP | **C3 done** |
| 24 | Лопата | Close interface, base cursor `7`, sound 10; **not consumed**. While carried in the 32 inventory slots it allows buried WorldObject records (negative money) to be collected on arrival (`FUN_00421B50` → `FUN_00433A8C(0x19)`) | **E done** |
| 25 | Правая часть амулета | Combine with 26→41, 27→42, 43→40 | **C3 done** |
| 26 | Левая часть амулета | Combine with 25→41, 27→43, 42→40 | **C3 done** |
| 27 | Нижняя часть амулета | Combine with 25→42, 26→43, 41→40 | **C3 done** |
| 28 | Берестяная грамота | Set `B7` slot 0 active; message «Ты прочитал Берестяную Грамоту» | **C2 done** |
| 29 | Медная лампа | No switch case | — |
| 30 | Медная лампа с именем Короля скелетов | Kill every other group's living actor in the location + corpse loot | **C4 done** |
| 31 | Факел | `_DAT_0081D628 = 1` → highlight ground markers in `FUN_0042532C`; reset at map load | **C2 done** |
| 32 | Неразменный пятак | No switch case | — |
| 33 | Кукла | Every living actor of every other group in the location gets `Entity+0xD2 = 0x13`; consumes | **done (D2)** |
| 34 | Заячий хвост | No switch case | — |
| 35 | Свисток | Move object-family actors to random free adjacent cells; reusable | **C4 done** |
| 36 | Чаша воеводы | Charisma current +10 | **C2 done** |
| 37 | Дудочка | Same as 35 | **C4 done** |
| 38 | Святые мощи | `+0xD3 = 0x46` (quest/state byte) | **C2 done** |
| 39 | Детские игрушки | Charisma current +5 | **C2 done** |
| 40 | Амулет Дракона | Result item (no direct case) | — |
| 41 | Левая и правая части | Combine with 27→40 | **C3 done** |
| 42 | Правая и нижняя части | Combine with 26→40 | **C3 done** |
| 43 | Левая и нижняя части | Combine with 25→40 | **C3 done** |
| 44 | Ключ Воды | Loc 50 + cell (34,11) → teleport 46,68,13,2 | **C4 done** |
| 45 | Перо Жар-птицы | No switch case | — |
| 46 | Клык белого волка | No switch case | — |
| 47 | Паутина странного паука | No switch case | — |
| 48 | Медное зеркало колдуна | Flip negative money on current-location ground records (reactivate) | **C4 done** |
| 49 | Волшебная вода | No switch case | — |
| 50 | Ларец колдуна | No switch case | — |
| 51 | Камень | No switch case | — |
| 52 | Ключ Луны | Loc 48 + cell (23,9) → teleport 49,106,17,2 | **C4 done** |
| 53 | Ключ Огня | Loc 49 + cell (15,17) → teleport 50,134,12,2 | **C4 done** |

## Shovel (subtype 24) — digging

The shovel is **not** the dig action. Buried treasures are ordinary `Game.N`
WorldObject records (`+0x21900`, 500 × `0x40`) whose signed money `+0x10` is
negative; the shovel is a *carried* requirement. Full trace, the 92-spot table
and the remake mapping: `DIG_SYSTEM.md`.

- `FUN_00414DE4` case `0x18` sets `_DAT_0081D610 = 0`, `_DAT_0081D660 = 7`,
  plays sound 10 and does not consume the shovel.
- `FUN_0042260C` resets the cursor to `4` on the next world click, so the shovel
  has no per-use dig code and no table lookup.
- `FUN_00421B50` collects a marker only when `money >= 0` **or**
  `FUN_00433A8C(0x19) != 0` — the latter scans the leader's 32 inventory slots
  for class `0x0B` subtype `0x18` (24).
- `FUN_0042532C` / `FUN_00428740` / the hover branch `0x423CD2` hide markers
  with negative money; the copper mirror (48) flips them for the current
  location, which is the "enlightenment" effect.

## Package plan

- **C1 (done):** `app/class11.rs` dispatcher; subtype 0 routed; 24 and all other
  subtypes inert; tests.
- **C2 (done):** books/identify/XP/charisma/quest flag/torch.
- **C3 (done):** duplicator, sword and amulet combinations, held-item-on-target routing (`try_craft_held_at`).
- **C4 (done):** revive, magnet, orbs/keys teleports, lamp, mirror, whistle/pipe.
- **D1 (done):** signed item weight end-to-end (`ItemInfo.weight: i32`,
  `item_weight`/`carried_weight`/`capacity: i32`); subtype 11 subtracts 10 kg.
- **D2 (done):** subtype 33 «Кукла» writes `Entity+0xD2 = 0x13`.
- **D3 (done):** this document + `AGENTS.md`.
- **E (done):** digging — buried WorldObject records (negative money) stay in
  the active pool, are never drawn, and are collected only with the shovel in
  the bag; the shovel arms cursor frame 7 and is not consumed. See
  `DIG_SYSTEM.md`.

## Subtypes with no switch case

`11, 29, 32, 34, 40, 45, 46, 47, 49, 50, 51` never reach `FUN_00414DE4`. They
are quest tokens handed out and taken away by the dialogue opcode ranges
`0xA6..=0xDB` / `0x70..=0xA5` (class `0x0B` items), so they need **no** use
handler. Only two of them are referenced elsewhere: `34` «Заячий хвост» by
quest opcode `0x158` (implemented) and `40` «Амулет Дракона», which is the
craft result of `41/42/43` + the matching part. `11` is passive (see above).

## Passive artefacts

The only class-11 item with a constant carried effect is `11` «Сын Луны»: its
signed template weight is `-10000`. Two functions read the class-11 template
table for stats (`FUN_0041B0C4` single weight, `FUN_0041B7E4` total weight);
armor/attack/party-size only read equipment slots, which class-11 items can
never occupy. There is therefore **no** other passive buff/debuff artefact in
this build.
