# Konung trade / exchange system

Target: `konung.res` (`sha256 d698c4f231489a372cc25212ab2819720a36d7ce5307a045db684135cf679810`).
Evidence: REA/Ghidra decompiles of `0x431E2C`, `0x430D6C`, `0x42260C`,
`0x4203F4`, `0x41AE00`, `0x41B5A8`, `0x41B2A8`, `0x41B0C4`, `0x41182C`,
`0x4175FC`, `0x4179C8`, plus the UI tables listed below.

## 1. Entry — dialogue opcode `0x113`

`FUN_00431E2C` is the **only** opcode that opens an inventory exchange. There are
no separate buy/sell/exchange opcodes: all four transaction cases happen inside
interface mode 7.

1. `_DAT_0081D694 = player.money`; add **+10 per class `0x0B` subtype `0x20`**
   inventory item.
2. Copy `player.inventory[32]` (`Entity+0x72`, `0x40` bytes) to trade row 0.
3. Resolve partner role with `FUN_0041182C`:
   - `0` = no owned waypoint; `1..5` = owner slot of the first five settlement
     waypoints (`Game.N Settlement +0x3D0`, stride 8);
   - roles `2`, `3`, `4` use settlement stock `+0x412` (24), `+0x444` (32),
     `+0x484` (23); money `99999`; when `Settlement+0x4B2 & 4`, filter class > 2;
   - roles `0/1/5` use the NPC's own `Entity+0x34` money and `+0x72` inventory.
4. `FUN_00430D6C(0)` sets interface mode **7** and draws the exchange.

## 2. Trade grid (`0x81CB24`)

Four contiguous 32-slot rows, stride `0x40` (`u16` item instances):

```text
row 0  0x81CB24  player bag
row 1  0x81CB64  player offer
row 2  0x81CBA4  partner stock / bag
row 3  0x81CBE4  partner offer
```

`0x81CC24[row]` is a per-row page offset (five visible cells per row).
`0x81D694/0x81D698` are available player/partner money; `0x81D6F4/0x81D6F8`
are money on the table; `0x81D714/0x81D718` are the offer totals.

Input (`FUN_0042260C`, mode 7):

| code | action |
|---|---|
| `(row << 16) \| (col << 8) \| 0x32` | move visible cell to the paired row (`0↔1`, `2↔3`) |
| `row << 8 \| 0x78` | page row left |
| `row << 8 \| 0x79` | page row right (only if an item exists at `page+5`) |
| `0xC8` | cancel (`FUN_004203F4(0)`) |
| `0xC9` | confirm (`FUN_004203F4(1)`, then reload the exchange) |

## 3. Prices (`FUN_0041AE00` / `FUN_0041B5A8` / `FUN_0041B2A8`)

`sell_value(total)`:

```text
round(total * 0.5 * (1 + (player_trade - npc_trade) * 0.008)
      * (role_sign > 0 ? 2/3 : 1))
```

`buy_value(total)`:

```text
round(total * (1 - (player_trade - npc_trade) * 0.008)
      * (role_sign > 0 ? 1.5 : 1))
```

`role_sign` is the role for roles 2..4 when `Settlement+0x4B2 & 0x10` is set
(negative otherwise); roles 0/1/5 use `+6`. Flag `0x10` means the settlement is
**independent / not yet yours**; conquest `0x16D` and force defeat clear it, which
is why a player-controlled village uses the negative price multiplier. Constants:
`0x450024` = 0.008,
`0x45002C` = 0.5, `0x450034` = 2/3, `0x4500C4` = 0.008, `0x4500CC` = 1.5.

`FUN_0041B2A8` per-instance value:

- prototype `price` at `+0x36` (stride `0x40`) or `+0x78` (stride `0x80`);
- classes `0..=8`: add condition bonus `u16@0x458530 + condition*0x10` when
  `condition >= 0 && flags & 1 == 0`, then clamp to at least 1;
- class 12: `price * quantity` (`+0x04 u32`);
- class 9: `price << 3` (negative special case uses `round(-qty*price) << 3`).

## 4. Money auto-balancing and transaction cases

`FUN_00430D6C` balances on every redraw:

```text
player_side  = sell_value(player offer) + player table money
partner_side = buy_value(partner offer)  + partner table money
```

- `partner_side < player_side`: refund player table money, then move partner
  money onto the table.
- otherwise: refund partner table money, then move player money onto the table.

This produces:

| case | result |
|---|---|
| buy | partner item → partner offer; player table money = buy value |
| sell | player item → player offer; partner table money = sell value |
| exchange | both sides offer; no cash when values match |
| exchange + additional pay | difference is moved automatically; confirm fails if the player cannot cover it |

## 5. Commit (`FUN_004203F4`)

Validation:

- carried item count `< 33` → otherwise `"Мешок у тебя не резиновый!"`
  (`0x45BD1C`);
- weight `<= (endurance_base*1000)/3 + 10000` → otherwise
  `"Превышен вес"` (`0x45C51C`);
- `sell_value + player_table >= buy_value + partner_table` → otherwise
  `"Слишком мало денег!"` (`0x45BF1C`).

Apply:

- player money `+= partner_table - player_table` (clamped ≥ 0);
- player inventory = partner offer + player bag (compacted);
- normal NPC: NPC money `+= player_table - partner_table`; NPC inventory =
  player offer + partner bag;
- settlement merchant: bought items are erased from the stock array; **sold
  items are destroyed** (`instance class = 0xFF`) and are not added to stock;
- both sides' trade skill `+= (player_table + sell_value) / 1024`, cap 100;
- a successful NPC deal reloads the exchange (`0xC9` path).
- cancel (`FUN_004203F4(0)`) returns player offers to the bag and leaves the
  partner unchanged.

## 6. Interface (`FUN_00430D6C`)

- Tile `INTR[5]` (100×100) over the gameplay viewport:
  640 = `(112,0,528,444)`, 800 = `(140,0,660,555)`.
- `INTR[6]` window at `(112,0)` / `(140,0)`.
- Four row rectangles (`DAT_00465E94`):
  - 640: x `230..635`, y `40/120/275/350`, height 60;
  - 800: x `288..794`, y `50/150/358/438`, height 60.
- `INTR[17]` 70×70 slot cells, step 69, five visible cells; `INTR[18..21]`
  left/right page arrows (`19/21` active variants).
- Buttons: `INTR[180]` at the right middle rect (code `200` = cancel),
  `INTR[182]` at the left middle rect (code `201` = confirm).
- Six money/total labels at `DAT_00465F34/0x465F54`; NPC name centered at top;
  hovered cell shows the item name (`FUN_0042F5F8`).

## 7. Merchant restock

Stock arrays are initially empty in all shipped `Game.0/1/2` saves.

- **Role 2** (`FUN_004175FC`, producer entity `+0x3DA`): timer `+0x4`
  (`0x5A0` frames), quota words `+0x40C/+0x40E/+0x410`, free slots in the
  combined `+0x40C` array (`5..0x1B`), herb/potion production and pair crafting.
- **Role 4** (`FUN_004179C8`, producer entity `+0x3EA`): merchant smithing
  `Entity+0x4C` gates recipe levels from `0x45A2FC`, `0x459ABC`, `0x45A0FC`;
  pending slot `+0x4B4`, timer `+0x8`; classes 12/2/0/1/3/5/4 by slot.
- **Role 3** (`+0x3E2`) has **no producer**: the per-frame settlement tick
  `FUN_0041D048` calls only `FUN_004175FC` (role 2), `FUN_004179C8` (role 4),
  and `FUN_00417F84` (role 5/training); no code writes `+0x444`, and every
  shipped `Game.N` role-3 array is zero. Role 3 is therefore a faithful
  **buy-only** merchant/fence.

## 8. Remake implementation

- `web/src/trade.rs` — pure model: four rows, paging, prices, auto-balance,
  confirm/cancel, outcomes.
- `web/src/trade_screen.rs` — original INTR layout/hit-testing/rendering.
- `web/src/app/trade.rs` — `start_trade` from opcode `0x113`, commit/cancel,
  settlement/NPC application, skill gain.
- `web/src/app/settlement_trade.rs` — role-2/role-4 restock, including
  `FUN_004179C8` full-stock distribution of existing gear onto settlement group
  members (lower-subtype gear is destroyed, the emptied slot is queued for
  replacement).
- `web/src/population/*` — `SettlementTrade` parser (`stock2/3/4`, producers,
  timers, quotas) and `Settlement::role_of`.
- `web/src/item.rs` — `instance_trade_value` (`FUN_0041B2A8`).
- Tests: `web/src/trade/tests.rs`, `web/src/app/tests/trade.rs`;
  headless: `web/tools/trade_smoke.mjs`.

## 9. Unknowns / approximations

- Role-2 simple herb quality is a fixed value; original recipe RNG/quality is
  approximated. Role-4 per-item RNG quality/smithing roll is also approximated.
- Ground/container `partner == 0` commit path (`FUN_004203F4`) is not wired to
  gameplay yet.
- Button sprite labels (`INTR[180]`/`[182]`) should be verified visually.
