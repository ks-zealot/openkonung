# Konung sound system — reverse-engineered notes

Target: `konung.res` (code) + `SOUNDS.RES` / `Sounds_.res` (data).

## 1. Files

Two variants of the same archive share one container format:

| File | Size | Occupied clips |
|---|---|---|
| `SOUNDS.RES` | 21 981 240 | 391 |
| `Sounds_.res` | 13 314 264 | 244 |

They are **not identical**: 197 ids are common (145 byte-identical, some differ in
size) and each has ids the other lacks. The game opens `SOUNDS.RES`
(`FUN_004392B4`, string `0x450690`).

## 2. Container format

```text
u8  magic[4]   = 00 00 00 01
u32 header_word                    // unresolved
Entry index[1000]                  // at file offset 8, 8 bytes each
    u32 data_offset                // relative to DATA_BASE
    u32 data_size
data_region[...]                   // raw signed 16-bit little-endian MONO PCM
```

The index occupies `8..8008`; the **data region starts at file offset 8008**
(`DATA_BASE`). `data_offset == 0xFFFFFFFF` marks an unused slot. Each clip is raw
`s16` mono PCM; its byte length is `frames * 2`.

Verified against `SOUNDS.RES`: id 0 = offset 0, size 66298; ids are contiguous;
the NPC voice ids `0x0C..0x13` are all present and frame-aligned.

## 3. Engine + playback

- **DirectSound** (`DSOUND.dll`, `DirectSoundCreate`). The engine object lives at
  `_DAT_0082D244` (vtable); `FUN_00429C0C` initialises it and creates **three**
  secondary buffers with `WAVEFORMATEX` records at `0x82D28C` / `0x82D29E` /
  `0x82D2B0`:
  - `nSamplesPerSec` **22050**, `nAvgBytesPerSec` 44100, mono, 16-bit
  - `nSamplesPerSec` **19050**, `nAvgBytesPerSec` 38092, mono, 16-bit
  - `nSamplesPerSec` **25050**, `nAvgBytesPerSec` 50100, mono, 16-bit
  The three rates are effectively **pitch variants**; playback selects one
  (`FUN_0042A13C` chooses buffer 0 for low non-UI ids, else `rng()%3`).
- **Playback** `FUN_0042A13C(id, x, y, z)`: locks the chosen buffer, copies the
  clip's PCM from the preloaded block `0x81A990[id]` (size `0x6213DC[id]`), sets
  volume/pan (`param_3`/`param_2`) and starts it. Up to 45 concurrent buffers.
- **Shutdown** `FUN_0042987C`.
- The interface loader `FUN_004392B4` preloads ids **0..55** into `0x81A990`.
  NPC greeting voice ids are `0x0C..0x13` (selected in `FUN_00435290` by the NPC
  archetype: humanoids `rng()%4 + 0x0C`).

## 4. Event ids (verified call sites)

There are **22 direct `E8` calls** to `FUN_0042A13C` in `konung.res`; there are
no function-pointer references. The compiled remake table lives in
`web/src/sound/events.rs`.

| site | handler | id |
|---|---|---|
| `0x43687e` | main-menu click (`FUN_00436214` state 1) | `4` |
| `0x435dca` | map toggle (`M`) | `4` |
| `0x422914` | HUD map click | `4` |
| `0x4242e6` | quest-log toggle | `4` |
| `0x424362` | character-screen toggle | `4` |
| `0x42146c` | engage (`FUN_00421458`) | `4` |
| `0x421568` | stance (`FUN_00421554`) | `4` |
| `0x4216f0` | weapon-slot cycle (`FUN_004216dc`) | `4` |
| `0x422480` | halt (`FUN_0042246c`) | `4` |
| `0x41e8a8` | lift/put-down held item | `4` |
| `0x41e0e9` | in-place item use | `9` |
| `0x415330/54a0/5bdc` | class-11 map/scroll item (`FUN_00414de4` cases `0x14`/`0x18` + common tail) | `10` |
| `0x41e680` | equip main slot (`FUN_0041e450`) | `11`, or `1` when incoming/displaced class is `2` |
| `0x41e872` | equip additional slot (`FUN_0041e6bc`) | `0` |
| `0x421e1c` | facing dodge (`FUN_0041c3cc` return `-2`) | `6` |
| `0x4144fd/0x414775` | actor action row (`FUN_0041435c`) | human table `0x4146e8` / monster table `0x414480` |
| `0x412abc` | monster action row 4 (ranged) | `arch*8 - 0x1CF` (bank slot `+1`; absent clips) |
| `0x4365d4` | gameplay ambience, 2% per frame | `0x100 + (loc-1)*8 + rand()%8` |
| `0x43538f` | dialogue open (`FUN_00435290`) | `0x0C..0x13` |

Actor action tables (`Entity+0x00`; `roll = rand()%100`):

- human (`0x4146e8`): death `34`; idle `35` only when `roll < 95`; attack `36`;
  walk `7` only for the leader; ranged `3`; melee `2`, or `8` when
  `Entity+0x12 == 2`.
- monster (`0x414480`): bank `arch*8 - 0x1D0`; `+0` melee, `+1` ranged,
  `+2` death, `+3` idle/walk (`0`: `roll < 95`, `1`: `roll < 75`), `+4` attack.

The archive has 391 occupied clips. 387 are referenced by one of these paths;
`42/44/50/52` are unused palette-1/2 human death/attack variants (all shipped
humans are palette 0). 70 referenced ids have no clip (for example human walk
`7` and idle `35`); `FUN_0042A13C` checks the size and stays silent.

## 5. Remake integration

- `web/src/sound.rs` decodes the archive (`SoundArchive::parse`, `entry`,
  `clip_bytes`, `frame_count`, `duration_ms`); constants `DATA_BASE = 8008`,
  `SAMPLE_RATES = [22050, 19050, 25050]`, `BITS = 16`, `CHANNELS = 1`.
- `web/src/sound/events.rs` holds the verified id tables; `app/sound.rs` decides
  when to queue them.
- Loaded as `KIND_SOUNDS = 14`; manifest field `sounds`.
- The runtime queues requested clip ids in `PlayableApp.sounds` / `Frontend.sounds`
  (one frame; cleared at the start of `step`, drained before the next step). A
  dialogue open pushes the NPC voice id.
- Chance rolls (ambient, monster idle/walk) use `PlayableApp.sound_rng`, a
  cosmetic LCG separate from `combat_rng`, so sound placement cannot change
  combat results.
- ABI: `sound_count`/`sound_id` (active frontend/game queue) and
  `debug_sound_clip_ptr/len/rate` (PCM access). The browser shell
  (`www/app.js`) builds a mono `AudioBuffer` on first use (base rate 22050) and
  plays it via WebAudio; the context is created at boot and resumed on the first
  pointer/key gesture (and on every playback attempt), because browsers start it
  suspended without a gesture.

## 6. Positional mix and buffer rates (implemented)

`src/sound/position.rs` ports the two original helpers:

- pan `FUN_00438DC4`: `0` when the source is on the player's row, otherwise
  `round_ties_even((source_y - player_y) * 10000 * 0.00625)` (the FPU `FISTP`
  round-to-nearest-even mode).
- volume `FUN_00438E24`: project both cells with `FUN_00438A78`, take the
  rounded Euclidean distance, then `dist * -10000 / 1600` for `dist <= 1600`,
  else `-10000`. `FUN_0042A13C` drops any request below `-4000`.
- buffer rate: `FUN_00429C0C` creates three mono 16-bit buffers at
  `22050/19050/25050` Hz. `FUN_0042A13C` always uses buffer 0 for `id < 0x20`
  except ids `2,3,6,7,8`; every other id picks `rand()%3`.

`PlayableApp.sounds` is now `Vec<SoundEvent { id, volume, pan, rate }>`.
`queue_sound_at(id, cell)` applies the positional mix; centred UI/item/voice
requests use `queue_sound(id)`. The ABI adds `sound_pan`, `sound_volume` and
`sound_rate`; `www/app.js` applies `StereoPannerNode` (`pan/10000`),
`GainNode` (`10^(volume/2000)`) and `playbackRate = rate/base`.

## 7. Unknowns / deferred

- `header_word` semantics (the `u32` after the magic).
- Monster idle/walk clips are emitted from combat action cycles only; the
  non-combat idle loop is not replayed.

