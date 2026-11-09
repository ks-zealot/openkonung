# Konung — function inventory and CLIENT.EXE triage

Target: `konung.res` (PE32/i386, image base `0x400000`,
`sha256 d698c4f231489a372cc25212ab2819720a36d7ce5307a045db684135cf679810`).

Full machine-readable inventory: `evidence/konung_res_functions.csv`
(one row per procedure; columns `address,name,kind,region,role,evidence_id`).

## 1. Method and coverage

- Provider: REA / Ghidra 12.1.4, default auto-analysis, complete procedure list
  (`list_procedures`, evidence `ev_b5e39e88c2b09068bbbe374a4a2b9a297af606f5cbb3de61547621d299da513e`).
- Addresses are Ghidra virtual addresses in `konung.res`.
- 627 total entries:
  - **72 external imports** (`EXTERNAL:*`, Win32/DirectX/DSound/PlayAVI/WinMM);
  - **43 import thunks** in the stub region (`0x445Cxx`–`0x445Dxx`);
  - **512 internal procedures**: 291 in the game code range
    (`0x410000`–`0x43DFFF`), 221 in the CRT/library range
    (`0x43E000`–`0x445E00`).
- Role classification:
  - **309 named game functions** — recovered from this investigation and the
    existing contracts (`KONUNG_SOURCE_OF_TRUTH.md`, `KEY_INPUT_SYSTEM.md`,
    `MOUSE_INPUT_SYSTEM.md`, `DIALOG_SYSTEM.md`, `SOUND_SYSTEM.md`);
  - **203 CRT/library range** — classified only by address range as runtime
    support (not individually reversed);
  - **0 unclassified game functions** — explicitly **UNKNOWN** roles; the
    call graph is present in the binary but not yet assigned semantics.
  - the quest/dialogue opcode table at `0x46601C` maps opcodes `0x112..0x16E`
    to 93 handlers; 51 of them were previously unclassified and are now labelled
    `quest opcode 0xNNN handler` in the CSV.
- This is a complete *inventory*, not a complete semantic audit of every body.
  The state-machine, startup, input and save/load systems were deep-dived; the
  No unknown game functions remain.

## 2. Core addresses (named)

### Startup / frame / input

| address | name | role |
|---|---|---|
| `0x43F4B4` | `entry` | CRT stub → WinMain pointer |
| `0x44016C` | `FUN_0044016C` | CRT startup |
| `0x42B1A8` | WinMain (not a Ghidra function) | init, window, config, message loop |
| `0x42B65C` | WndProc (not a Ghidra function) | Win32 messages |
| `0x436214` | `FUN_00436214` | per-frame driver + screen state machine |
| `0x435B0C` | `FUN_00435B0C` | keyboard command dispatcher |
| `0x42260C` | `FUN_0042260C` | mouse/UI dispatcher |
| `0x4381C8` | `FUN_004381C8` | mouse command compiler |
| `0x428C00` | `FUN_00428C00` | cursor delta integration/clamp |
| `0x42A470` | `FUN_0042A470` | frame compositor + software cursor |
| `0x42ADE4` | `FUN_0042ADE4` | movie player (`PlayAVI.dll`) |
| `0x42B098` | `FUN_0042B098` | movie message pump |

### Screen / interface

| address | name | role |
|---|---|---|
| `0x42CBD0` | `FUN_0042CBD0` | interface renderer + mode setter |
| `0x4304F4` | `FUN_004304F4` | save/load slot list draw |
| `0x43060C` | `FUN_0043060C` | apply resolution/video mode |
| `0x42C5A0` | `FUN_0042C5A0` | save/load screen init |
| `0x42EA6C` | `FUN_0042EA6C` | paper-doll/character render |
| `0x42F578` | `FUN_0042F578` | character-screen control handler |
| `0x430D6C` | `FUN_00430D6C` | inventory-detail screen opener |
| `0x43ABFC` | `FUN_0043ABFC` | MENU page image loader/draw |

### Save / load

| address | name | role |
|---|---|---|
| `0x4246C0` | `FUN_004246C0` | load `KONUNG.SA0..9` |
| `0x424B3C` | `FUN_00424B3C` | save `KONUNG.SAV` + `KONUNG.SA0..9` |
| `0x43ECA7` | `FUN_0043ECA7` | CRT open wrapper |
| `0x43EE9E` | `FUN_0043EE9E` | CRT read |
| `0x43F16F` | `FUN_0043F16F` | CRT open-for-write wrapper |
| `0x43F186` | `FUN_0043F186` | CRT write |
| `0x43F121` | `FUN_0043F121` | CRT close |
| `0x43F57C` | `FUN_0043F57C` | capture local time (`GetLocalTime`/`mktime`) |
| `0x43F5ED` | `FUN_0043F5ED` | asctime builder (`Www Mmm dd hh:mm:ss yyyy`) |
| `0x43F720` | `FUN_0043F720` | asctime-style save-name formatter |

### World / map / population

| address | name | role |
|---|---|---|
| `0x43ADF0` | `FUN_0043ADF0` | map/location loader (mode 0 new / 1 load) |
| `0x4392B4` | `FUN_004392B4` | asset loader (GRAPH/INTR/SOUNDS/save names) |
| `0x43A75C` | `FUN_0043A75C` | FLAMES.RES loader |
| `0x43A870` | `FUN_0043A870` | GAME_x + QUEST_R.RES loader |
| `0x420A8C` | `FUN_00420A8C` | population growth constructor |
| `0x421B50` | `FUN_00421B50` | per-frame group/actor dispatcher |
| `0x4123BC` | `FUN_004123BC` | NPC/monster action machine |
| `0x412E00` | `FUN_00412E00` | object/enemy idle AI |
| `0x415FC8` | `FUN_00415FC8` | citizen group AI dispatcher |
| `0x4168A4` | `FUN_004168A4` | citizen waypoint/wander movement |
| `0x41ECC8` | `FUN_0041ECC8` | actor logic (walk/idle/target) |
| `0x4142D4` | `FUN_004142D4` | issue move (dest+route+wake) |
| `0x414DE4` | `FUN_00414DE4` | item/spell action dispatcher (act-verb switch) |
| `0x41BF44` | `FUN_0041BF44` | spawn projectile/effect into `0x80B0D0` pool |
| `0x420F30` | `FUN_00420F30` | effect/projectile update + contact damage |
| `0x428390` | `FUN_00428390` | world-map discovered-location draw |
| `0x4353B0` | `FUN_004353B0` | camera follow/scroll (`_DAT_0081D564`) |
| `0x41435C` | `FUN_0041435C` | actor step/animation commit |
| `0x43DA1F` | `FUN_0043DA1F` | route builder over collision grid |
| `0x438A78` | `FUN_00438A78` | entity → local pixel projection |
| `0x438AB4` | `FUN_00438AB4` | screen → cell inverse projection |

### Items / character / dialogue / sound

| address | name | role |
|---|---|---|
| `0x41E450` / `0x41E6BC` / `0x41E8A8` | equip/pickup handlers |
| `0x424E0C` | `FUN_00424E0C` | item-class → cursor/action map |
| `0x435290` | `FUN_00435290` | open dialogue for NPC |
| `0x431820` / `0x432954` / `0x431E2C` | recruit / dismiss / trade opcodes |
| `0x429C0C` | `FUN_00429C0C` | DirectSound init |
| `0x42A13C` | `FUN_0042A13C` | sound play |

## 3. Unclassified game functions

The CSV marks **0 internal game functions** as `unclassified`. They are not
guessed. They are clustered mainly in:

- `0x41xxxx`–`0x42xxxx` — combat/AI/item/interface helpers;
- `0x43xxxx` below `0x43E000` — loaders, render helpers, quest script helpers.

To promote one, use the recorded evidence style:
`analyze_function` → `procedure_callers/callees` → `procedure_pseudo_code`,
then update the CSV role and add a section to the relevant document.

## 3.1 Follow-up resolutions (unknowns closed)

| unknown | resolution | evidence |
|---|---|---|
| `_DAT_0081D564` | camera follow target entity (`0` = free camera); Ctrl+S toggles; `FUN_004353B0` follows/scrolls | `ev_cfe0de6c…`, `ev_190e0999…` |
| `_DAT_0081D6A0` + `0x80B0D0` | 100 × `0x20` projectile/effect pool + active count; `FUN_0041BF44` spawns, `FUN_00420F30` updates/contact-damages | `ev_37f84b5a…`, `ev_17912016…` |
| `0x81CF80` | 56 discovered world-map location flags; `FUN_00428390` draws the map; mode 5 is travel, not inventory | `ev_a6022cfc…`, `ev_157d05a8…` |
| location names | `0x45B394` = 56 CP866 names, stride `0x28` (index 0..55); also drives save defaults | `evidence/konung_location_names.csv` |
| `_DAT_0081D764` | write-only debug selector; no reader in the binary | `ev_dba63d75…` |
| `_DAT_0081D6B8` 1/3 | no writer exists; unreachable as menu states (MENU image 1 = settings page, separate index) | `ev_340a83ec…` |
| six option labels | Прозрачность, Смена дня и ночи, Световые эффекты, Музыка, Тени, Звук | settings page `menu640_001.png` |
| save-name composition | location default + space + formatted date/time; trailing char stripped | `0x4368C1`–`0x4369BA` |
| `FUN_004123BC` graph | idle 0/6, walk 1/7, attack 2/8, death 3/9, ranged 4, melee 5 | `ev_fbc9cb35…` |
| PlayAVI threading | one helper thread via `FUN_004281F0` + thread-local `WH_GETMESSAGE` hook; thread body `0x428150` waits on an event with 100 ms timeout and calls video frame render `0x429FE4` | playavi `ev_714d0e8b…`, `ev_92049848…` |
| save-name date format | asctime string (`Www Mmm dd hh:mm:ss yyyy\n`) truncated at the `:` before seconds → `Www Mmm dd hh:mm` | `FUN_0043F57C`/`FUN_0043F5ED` |
| music/sound split | `_81D60C` = music (MCI `FUN_004296B0`/`FUN_00429714`); `_81D62C` = sound effects (gates `FUN_0042A13C`) | `ev_4bd4b358…`, `ev_f3f28e6e…` |
| `0x03B9` message | `MM_MCINOTIFY` from the MCI music device; `wParam == 1` → `FUN_00429630` resumes music | `ev_715a2760…` |

No unclassified game functions remain.

Quest opcodes: `0x46601C + (opcode-0x112)*4` is the handler pointer table
(93 entries, `0x112..0x16E`). Known specific handlers keep their names
(`0x113` trade/loot, `0x123` recruit, `0x14C` dismiss); the rest are labelled
`quest opcode 0xNNN handler` in `evidence/konung_res_functions.csv`.

## 4. `CLIENT.EXE` triage (not the remake target)

| property | `konung.res` | `CLIENT.EXE` |
|---|---|---|
| SHA-256 | `d698c4f2…9810` | `adf0a2793ee42b16dcf31c53799cf5c9b0ca37471873e1888c26e2b5f01bf4ac` |
| size | 1 586 688 | 939 008 |
| PE32/i386, base | yes, `0x400000` | yes, `0x400000` |
| segments | Headers, BEGTEXT `0x410000–0x445E00`, DGROUP, .bss, .idata `0x840000`, .reloc, .rsrc | Headers, BEGTEXT `0x410000–0x439800`, DGROUP `0x440000`, .bss `0x500000`, .idata `0x780000`, .reloc, .rsrc |
| procedures | 627 | 538 |
| strings | 176 | 174 |
| entry | `0x43F4B4` | `0x433A72` |
| WinMain pointer | `0x42B1A8` | `0x42EBC8` |
| CRT startup | `0x44016C` | `0x435910` |
| window class | `"Konung"` @ `0x4632BC` | `"Konung_Client"` @ `0x44ED50` |
| save files | `KONUNG.SAV`, `KONUNG.SA0..9` | **absent** |
| dialogue file | `QUEST_R.RES` | `QUESTR.RES` (`"File QUEST.RES not found"`) |
| PlayAVI | imported | **absent** |
| network strings | none | `SERVERS.CFG`, `"Server failed"` |
| config | `KONUNG.CFG` | `KONUNG.CFG` |

**Conclusion:** `CLIENT.EXE` is a **separate, earlier/alternate client build**
(most likely the multiplayer/client variant: `SERVERS.CFG`, `"Server failed"`,
`Konung_Client` window class). It shares the CRT/stub conventions and the same
class of game code, but every address from `konung.res` must be re-derived.
It has no save-slot system and no PlayAVI movie path. It is **not** used by the
remake and was not deep-dived.

## 5. Evidence

- Full `konung.res` evidence bundle:
  `evidence/konung_res_rea_evidence_bundle.json` (146 records, 3.45 MB;
  includes the follow-up `konung.res`, `CLIENT.EXE` and `playavi.dll` sessions).
- CSV: `evidence/konung_res_functions.csv`.
- `CLIENT.EXE` triage evidence IDs:
  `ev_de15569ea6831285c6e5252f2af1f5aa5940d148db4e80db2a5c26295444a5a3` (overview),
  `ev_c7d025e93087580359d353e91e0579a67c6b8625322e6db427f7a349460ce3d2` (strings),
  `ev_5a98c14cc7e8808ec34890e3fec148e73179b0741b1a0b2cb15db7ca1beffb3d` (entry),
  `ev_20b6ae2c80e18b472156a3411d59bfa9f0870b9aeac11aa5ddfde053dbc05a1f` (network/QUEST strings).
