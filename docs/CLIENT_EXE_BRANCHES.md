# Konung multiplayer client — branch and gate catalogue

Target: `CLIENT.EXE` (PE32/i386, base `0x400000`,
`sha256 adf0a2793ee42b16dcf31c53799cf5c9b0ca37471873e1888c26e2b5f01bf4ac`).

This document summarises the complete branch extraction in
`evidence/client_exe_gates.csv` and calls out the gates that select behaviour
(not merely loop bounds). Addresses are `CLIENT.EXE`-specific.

## 1. Coverage

| item | count |
|---|---|
| conditional branches (`jcc`) | **3 555** |
| indirect jump tables | **17** |
| functions containing gates | **375** |
| branch mnemonics | `je 1293`, `jne 972`, `jge 355`, `jle 279`, `jl 177`, `jg 114`, `jbe 113`, `jb 112`, `jns 38`, `jae 37`, `js 31`, `ja 27`, `jp 3`, `jo 3`, `jecxz 1` |

For each row the CSV records the branch address, containing function, target,
the immediately preceding flag-setting instruction (`flag_source`,
`flag_mnemonic`, `flag_operands`) and every absolute global referenced by the
condition or target. This makes the catalogue usable without re-disassembling.

## 2. Most frequently gated globals

| global | gates | meaning |
|---|---|---|
| `0x757800` | 67 | interface mode (`0` world, `5` map/travel, `7` inventory detail) |
| `0x4F5C48` | 48 | DGROUP render/batch state (role unresolved) |
| `0x4F5C44` | 36 | DGROUP render/batch state |
| `0x6EB774` | 26 | **screen state** `0..0xB` |
| `0x4F5C40` | 24 | DGROUP render/batch state |
| `0x7577EC` | 22 | session/player state |
| `0x7577E8` | 20 | keyboard input enable (0 = accept) |
| `0x4F5C3C` | 20 | DGROUP render/batch state |
| `0x6EB79C` | 17 | input/interface timer or index (unresolved) |
| `0x4F5C2C` | 16 | DGROUP render/batch state |
| `0x4F5C84` | 15 | DGROUP render/batch state |
| `0x6EB78C` | 14 | session/queue state |
| `0x7578A0` | 14 | video depth (`0x10` = 16-bit, else 32-bit paths) |
| `0x6EB7B0` | 12 | **map/location id** (1-based) |
| `0x757784` | 11 | world/actor list state |
| `0x6EB7A4` | 10 | `CLANS.CFG` entry count |
| `0x7577AC` | 9 | mouse button state (bits 0/1) |
| `0x6EB760` | 9 | current interaction target |
| `0x6EB76C` | 7 | settlement record pointer |
| `0x6EB784` | 7 | pending interface action |

The `0x4F5Cxx` group is the largest unresolved cluster. It is in DGROUP and is
tested by the renderer and several UI helpers; it is most likely a DirectDraw /
sprite-batch cursor, but that is **inferred**, not proven.

## 3. Gate families

### 3.1 Startup (`WinMain` `0x42EBC8`)

| address | condition | effect |
|---|---|---|
| `0x42EC81` | `hwnd == 0` | abort with return 0 |
| `0x42ED15` | `GetDriveTypeA(letter) == 5` | accept CD-ROM and stop the A:..Z: scan |
| `0x42ED1E` | scan index `> 'Z'` | clear the CD-root buffer |
| `0x42EE35` | `GetUserNameA(...) == 0` | substitute `"Unknown"` as player name |
| `0x42EE5D` | video init result `== 0` | destroy window, return 0 |
| `0x42EE8B` | sound init result `== 0` | destroy window, return 0 |
| `0x42EEAD` | asset loader result `== 0` | destroy window, return 0 |
| `0x42EF1E` | app active `0x7786B4 == 0` | skip the frame tick |
| `0x42EF36` | `now - last_tick < 0x6EB778` | skip the frame (60 ms limiter) |
| `0x42EF5A` | running flag | exit the loop |

### 3.2 WndProc (`0x42EF70`)

| address | condition | effect |
|---|---|---|
| `0x42EF7C` | `quit flag 0x6EB780 != 0` | clear running, `DestroyWindow`, return 0 |
| `0x42F265`–`0x42F369` | message range tree | selects the handler for 1/2/0xF/0x1C/0x20/0x100/0x101/0x200/0x201/0x202/0x204/0x205/0x3B9/0x400 |
| `0x42EFBF` | `wParam == 1` for `MM_MCINOTIFY` | resume music |
| `0x42F1A1` | `state == 1` in `WM_PAINT` | redraw world + present |
| `0x42F22E` | app active | run `FUN_0042D094` + present |
| `0x42F0D6` | key in `{0x10,0x11}` on key-up | enqueue only Shift/Ctrl |
| `0x42F036` | `0x7577E8 != 0` | suppress key enqueue |

### 3.3 Screen state machine (`FUN_0042855E`)

| address | condition | effect |
|---|---|---|
| `0x42857F` | `input head == tail` | no event this frame |
| `0x4285B5` | event flag `== 0` (key-up) | only modifier handling |
| `0x428612` | `state == 1` **or** key `== 0x1B` (ESC) | zero the gameplay key |
| `0x4287B3` | `state-1 > 7` | skip the key-state switch |
| `0x429F74` | `state > 0xB` | no-op handler |
| `0x429F81` | state jump table `0x429F44` | dispatch `0..0xB` |

State-specific gates:

| state | address | condition | effect |
|---|---|---|---|
| 0 | `0x4288B5` | `pending == 0xA && session flag != 0` | enter state 0xA |
| 0 | `0x4288D3` | venues busy `0x6EB7BC == 0` | commit `pending` into the state |
| 2 | `0x428EBC` | any mouse button | return to state 9 |
| 3 | `0x428A7B` | role `0x7577E4 == 0 && idx < 2` | invalidate the first two menu items |
| 3 | `0x428A9F` | left button && `idx > -1` | run the 6-entry menu switch |
| 3 | `0x428EA4` | `idx > 5` | no-op |
| 4 | `0x428F1F` | left button | run the 33-entry editor switch |
| 4 | `0x429734` | `idx > 0x20` | no-op |
| 5 | `0x4297A3` | index-chain thresholds 5/8/9/0x12 | select the session action |
| 8 | `0x429B2B` … `0x429C0F` | option flags | toggle the six options |
| 8 | `0x429D4C` | `idx > 9` | no-op |
| 9 | `0x429DC6` | `pending == 8` | alternate start path vs `ME` loader |
| 10 | `0x429E0B` | none | unconditional state 1 |
| 11 | `0x429EEB` | none | unconditional state 5 |

### 3.4 Network (`FUN_0042D1EC`, `FUN_0042D574`, `FUN_0042CA4F`, `FUN_0042A00B`)

| address | condition | effect |
|---|---|---|
| `0x42D1EC` | no provider enumerated | return 0 |
| `0x42D21F` | `DirectPlayEnumerateA` result | continue/fail |
| `0x42D2C5` | `DirectPlayLobbyCreateA == 0` | create the lobby |
| `0x42D2??` | `QueryInterface == 0` | query the lobby view |
| `0x42D3??` | `CoCreateInstance == 0` | use the DirectPlay interface |
| `0x42D7EC` | `(flags & 1) == 0` | accept the provider GUID |
| `0x42D574` | `dp == NULL` | no receive |
| `0x42D59E` | `Receive == DPERR_NOMESSAGES` | no packet |
| `0x42D5B8` | `fromID == 0` | drop peerless packet |
| `0x42CA4F` | `*p & 0x80` | remove a venue record |
| `0x42CB01` | `type & 0x3F` | packet handler table `0x44896C` |
| `0x42A00B` | `frame - stamp > 0xF` | retry (16 frames) |
| `0x42A00B` | `retry > 0xF` | `"Server failed"` |
| `0x4284A0` | `out_len + size > 0x1FF` | flush before append |
| `0x4284A0` | queue flag `!= 0` | append to the reliable venue queue |

The `Server failed` gate is the only fatal network condition; it is reached
after 16 retries of an unacknowledged packet.

### 3.5 Data loaders

| loader | address | condition | effect |
|---|---|---|---|
| config | `0x4172FF` | `fopen(CLANS.CFG) != 0` | parse clan lines |
| config | `0x4172FF` | `fopen(SERVERS.CFG) != 0` | parse server lines |
| config | `0x4172FF` | `open(KONUNG.CFG) == -1` | defaults (flags 1, mode 0x111) |
| config | `0x4172FF` | video mode not `0x111`/`0x114` | force `0x111` |
| assets | `0x41BF8E` | `GlobalAlloc(0x1B00000,0x2800000) == 0` | "Not enough free memory" |
| assets | `0x41BF8E` | each `open() == -1` | log + fail |
| assets | `0x41BF8E` | `index[i].size > 1` | load the clip |
| assets | `0x41BF8E` | `offset < 0x80000000` | valid OBJECTS entry |
| assets | `0x41BF8E` | `video_depth == 0x10` | 16-bit conversion paths |
| assets | `0x41BF8E` | option `0x7577CC == 0` | skip FLAME decompression |
| maps | `0x4176F5` | `map_id` 1-based | seek `(id-1)*0x2F120` |
| maps | `0x4176F5` | terrain cell `0xFF`/low nibble `0xF` | collision encoding |
| flam | `0x41EB35` | `open(FLAMES.RES) == -1` | log + fail |
| ME | `0x41FF3F` | `open(ME<w>R.RES) == -1` | log `"<name> not found rc=16"` |

### 3.6 Quest (`FUN_00425E5C`, `FUN_004271A9`)

| address | condition | effect |
|---|---|---|
| `0x425E5C` | `opcode < 0x38` | return 0 |
| `0x425E5C` | `opcode < 0x70` | return 0 |
| `0x425E5C` | `opcode < 0xA6` | `FUN_00426B0A` |
| `0x425E5C` | `opcode < 0xDC` | `FUN_00425F7F` |
| `0x425E5C` | `opcode < 0x112` | `FUN_00426B31` |
| `0x425E5C` | `opcode >= 0x112` | table `0x44EA30`, 93 entries |
| `0x425E5C` | `table[i].has_text != 0` | pass the inline text to `FUN_004279B8` |
| `0x4271A9` | terminator `0xFFFF` | end of the condition program |
| `0x4271A9` | `term & 0x2000` | opcode predicate → `FUN_00425E5C` |
| `0x4271A9` | `term & 0x8000` | set vs clear a quest flag bit `0x80` |

### 3.7 UI / item / character gates

| gate | global | meaning |
|---|---|---|
| menu role | `0x7577E4` | client vs host; invalidates the first two menu items for clients |
| interface mode | `0x757800` | world / map / inventory-detail branches |
| modal flags | `0x75780C` | bits 0/1/2/3 gate input and interface requests |
| video depth | `0x7578A0` | 16-bit vs 32-bit sprite conversion |
| options | `0x7577CC`, `0x7577E0`, `0x7577F0`, `0x7577F8`, `0x757810`, `0x757818` | six settings |
| map id | `0x6EB7B0` | map selection and terrain scan |
| clan count | `0x6EB7A4` | `CLANS.CFG` line count |
| mouse | `0x7577AC`, `0x7577B0` | button state/edge |
| key enable | `0x7577E8` | suppress key enqueue |

## 4. How to read a gate row

Example (from `FUN_0042CA4F`, packet venue removal):

```text
address=0x42ca93  function=0x42ca4f  mnemonic=je  target=0x42cb01
flag_source=0x42ca90  flag_mnemonic=test  flag_operands=byte ptr [eax], 0x80
globals=0x5baae0
```

This means: at `0x42CA93`, jump to `0x42CB01` when the byte at `[eax]` does
**not** have bit `0x80`; otherwise the venue-removal block runs.

## 5. Verification notes and limits

- The catalogue is a static direct-branch inventory. It does not model indirect
  jumps other than the 17 detected tables, and it does not prove reachability.
- Conditions are attached from the immediately preceding flag-setting
  instruction. In hand-written assembly that is reliable in the sampled
  functions; a branch whose flags come from a call or a farther producer will
  show an empty or stale `flag_source`.
- `jmp-table` rows carry the table address in `target`; the index source is in
  `globals` when it is an absolute global.
- Packet-type and quest-opcode semantics remain partial; see
  `CLIENT_EXE_NETWORK.md` and `CLIENT_EXE_SAVE_LOAD_QUEST.md`.
