# Konung multiplayer client — function inventory and role map

Target: `CLIENT.EXE` (PE32/i386, base `0x400000`,
`sha256 adf0a2793ee42b16dcf31c53799cf5c9b0ca37471873e1888c26e2b5f01bf4ac`).

Machine-readable inventories produced by this investigation:

- `evidence/client_exe_functions.csv` — one row per procedure:
  `address,rea_name,label,kind,role,reachable,callers,callees,imports`
- `evidence/client_exe_gates.csv` — one row per conditional branch / jump table:
  `address,function,function_label,mnemonic,target,flag_source,flag_mnemonic,flag_operands,globals`

## 1. Method and coverage

- Provider: REA / Ghidra 12.1.4, default auto-analysis.
- Procedure inventory: `list_procedures`,
  `ev_c696c9a58980c2efb4b49bb4b9a55b58e45e8968b843c44827abcc87799b0d6b`.
- Direct-call graph reconstructed from all `E8 rel32` calls in
  `0x410000–0x4397FF`, with call targets resolved to their containing
  procedure.
- Reachability roots: the CRT startup (`0x435910`), the per-frame drivers
  (`0x42855E`, `0x42D094`), the asset/map/config loaders (`0x41BF8E`,
  `0x4172FF`, `0x4176F5`), plus every direct call in the raw WinMain
  (`0x42EBC8`) and WndProc (`0x42EF70`) regions.
- Gate extraction: every conditional jump plus every indirect jump table, with
  the flag-setting instruction (`cmp`/`test`/arithmetic) that immediately
  precedes it and the absolute globals referenced by either instruction.

Totals:

| class | count |
|---|---|
| procedure entries | **538** |
| internal procedures | **462** |
| external imports + thunks | **76** |
| reachable internal procedures | **361** |
| runtime/library, reachable | **199** |
| game, reachable | **162** |
| unreached library/dead | **95** |
| unreached game candidates | **6** |
| conditional branches | **3 555** |
| indirect jump tables | **17** |

The six “unreached game candidates” are all reached indirectly, not by a direct
`call`:

| address | reason |
|---|---|
| `0x426590`, `0x4265CB`, `0x426609` | quest opcode handlers `0x134`, `0x135`, `0x136` (table `0x44EA30`) |
| `0x426F6C` | quest opcode handler `0x160` |
| `0x42D7EC` | DirectPlay enumerate callback (invoked by `DPLAYX.dll`) |
| `0x42D83C` | callback/indirect-only helper (not in the quest table) |

This is a complete *direct-call* inventory; it is not a full semantic audit of
all 462 bodies. The startup, state machine, network, data-loader and quest
subsystems were deep-dived; the remaining runtime/library bodies are classified
by region and reachability only.

## 2. Core address map

### 2.1 Startup / frame / input

| address | label | role |
|---|---|---|
| `0x433A72` | PE entry | CRT stub → WinMain pointer `0x42EBC8` |
| `0x435910` | CRT startup | calls the WinMain pointer |
| `0x42EBC8` | WinMain (raw) | window, config, video, sound, assets, 60 ms loop |
| `0x42EF70` | WndProc (raw) | Win32 messages |
| `0x4306ED` | CRT startup helper | called from WinMain |
| `0x42CEC0` | time/random seed init | returns seed stored at `0x7577D0` |
| `0x42D094` | network/queue pump | input + state + receive + retry |
| `0x42855E` | per-frame input + state driver | consumes one input event, dispatches state |
| `0x42E19C` | frame compositor/present | DirectDraw blit + cursor |
| `0x41B97D` | video mode/table init | 640/800 tables + DirectDraw |
| `0x42DB60` | DirectSound init | called from WinMain |
| `0x42D788` / `0x42D69C` / `0x42D174` | shutdown helpers | `WM_DESTROY` |

### 2.2 Screen / interface

| address | label | role |
|---|---|---|
| `0x429F44` | state handler table | 12 entries, states `0..0xB` |
| `0x428A5E` | state 3 main menu | `Игра / Новый герой / Установки / Выход` |
| `0x428F02` | state 4 character editor | 33-entry index table |
| `0x429769` | state 5 session screen | DirectPlay join + map start |
| `0x429ADE` | state 8 settings | 10-entry index table |
| `0x429D6E` | state 9 preload | loads `ME<w>R.RES` |
| `0x429E0B` | state 10 start gameplay | commits state 1 |
| `0x429EEB` | state 11 new-game commit | copies hero template |
| `0x416F91` | UI hit-test | rectangle table → index |
| `0x422C76` | UI hover/highlight | |
| `0x422E07` | UI control action | |
| `0x42255A` | team/player list render | 10 slots at `0x5E812C`, stride 0x74 |
| `0x4169DA` | fatal/log | `CLIENT.LOG` + quit |
| `0x41FF3F` | `ME<w>R.RES` loader | builds `ME640R.RES` / `ME800R.RES` |

### 2.3 Network (DirectPlay)

| address | label | role |
|---|---|---|
| `0x42D1EC` | DirectPlay enumerate/lobby/join | creates the session |
| `0x42D574` | Receive | dequeues into `0x6EB154` |
| `0x42CA4F` | packet dispatcher | type table `0x44896C` |
| `0x4284A0` | reliable send queue | venues `0x5BAAE0`, stride 0x20B |
| `0x42D620` | flush | interface slot `+0x68` |
| `0x42A00B` | retry / `Server failed` | 16-frame retry, 16 attempts |
| `0x42D7EC` | enumerate callback | stores provider GUID |
| `0x41701E` | player-state packet (0x73 B) | config strings + hero fields |

### 2.4 Data loaders

| address | label | role |
|---|---|---|
| `0x4172FF` | config loader | `CLANS.CFG`, `SERVERS.CFG`, `KONUNG.CFG` |
| `0x4176F5` | `MAPS.RES` loader | 20 × 0x2F120 |
| `0x41BF8E` | asset loader | `SOUNDS_/GRAPH/LIGHTS/OBJECTS/GAME/QUESTR` |
| `0x41EB35` | `FLAMES.RES` loader | 74 frames |
| `0x41FDA7` | `PLAY<w>R.RES` loader / player screen | `0x41FDF0` |
| `0x41FF3F` | `ME<w>R.RES` loader | interface archive |
| `0x42013A` | `NEW<w>R.RES` loader / session reset | `0x420183` |
| `0x4202D2` | `INTO<w>R.RES` loader / game init | `0x420322` |

### 2.5 Quest

| address | label | role |
|---|---|---|
| `0x425E5C` | quest opcode dispatcher | `0x112..0x16E` via `0x44EA30` |
| `0x44EA30` | opcode table | 93 × `{u32 has_text, u32 handler}` |
| `0x4271A9` | condition-program evaluator | B4 at `0x554CC8` |
| `0x557BA8` | action program B5 | 120 000 bytes |
| `0x575068` | quest flags | 300 × u32 |

### 2.6 Runtime/library helpers (named from observation)

`0x4329F4` strcpy, `0x4328F0` memset, `0x432A19` strcat, `0x43389A` strlen,
`0x432945` rand, `0x433865` itoa, `0x43302C` open, `0x433223` read,
`0x433416` lseek, `0x432F48` close, `0x432C5D` fopen, `0x432D92` fclose,
`0x432F96` fgets, `0x433774` filelength, `0x433900` atoi, `0x432871` stack
probe (chkstk).

## 3. Comparison with `konung.res`

| property | `konung.res` | `CLIENT.EXE` |
|---|---|---|
| procedures | 627 | 538 |
| internal | 512 | 462 |
| external/thunks | 115 | 76 |
| save system | `KONUNG.SAV` + `SA0..9` | **absent** |
| movies | PlayAVI | **absent** |
| network | none | DirectPlay 3A |
| maps | `Konung.N` + `Lights.N` | `MAPS.RES` (20 × 0x2F120) |
| game data | per-location files | `GAME.RES` packed |
| config | 28-byte `KONUNG.CFG` | 124-byte `KONUNG.CFG` |
| quest script | `QUEST_R.RES` | `QUESTR.RES` (same 7-block engine) |
| UI archives | `INTR/MENU/...` | `PLAY/ME/NEW/INTO<w>R.RES` + `GRAPH.RES` |

`CLIENT.EXE` reuses the CRT, the Win32 skeleton, the actor/entity record
layout, the item/quest flag conventions and the 93-opcode quest range, but it is
a separate product with its own data and network layer.

## 4. How to use the CSV

- `evidence/client_exe_functions.csv` is the authoritative address→label map.
  `role` is a reachability/region classification, not a semantic claim.
- `evidence/client_exe_gates.csv` is the branch catalogue. For a gate, read
  `flag_source`/`flag_mnemonic`/`flag_operands` for the condition and `target`
  for the taken edge; `globals` lists the absolute memory operands involved.
- The 17 `jmp-table` rows identify the indirect dispatch tables (state machine,
  character editor, settings, quest, packet handlers).
- To promote an unlabelled function, follow the existing pattern:
  `analyze_function` → `procedure_pseudo_code` → `procedure_callers/callees`,
  then update the CSV label and the relevant subsystem document.

## 5. Residual unknowns

1. Semantic labels for ~290 reachable runtime/library procedures that were only
   region-classified.
2. Exact meaning of the `0x4F5Cxx` DGROUP globals that dominate the gate
   catalogue (DirectDraw/UI batch state; not decoded).
3. `0x42D83C` (indirect-only helper).
4. Full packet-type semantics (see `CLIENT_EXE_NETWORK.md`).
