//! Centralised constants for `timing` (extracted from the module sources).

pub const MAX_PATH_STEPS: usize = 0x1f;
/// Maximum placement attempts in the original routine.
pub const MAX_ATTEMPTS: u8 = 100;
/// Original movement is discrete: one collision cell per completed walk-animation
/// cycle (`FUN_0041435c` state 1/7 + `FUN_00413de4`). The world logic runs on the
/// original fixed frame budget: `timeGetTime()` deltas are compared against a
/// **60 ms** period (`konung.res` `0x42b32f` `MOV dword ptr [0x81d5c8],0x3c`;
/// the limiter compare is at `0x42b61a`), so the game advances one animation /
/// logic frame per 60 ms (~16.67 Hz). At default game speed `[0x81d634]` (0) the
/// entity update runs once per frame and the tick counter `_DAT_0081d640`
/// increments once per frame (`0x4366e5`); the periodic population pass keys off
/// `& 0xf` (~0.96 s per 16 frames), corroborating the ~16.7 Hz rate. A walk cycle
/// of `n` frames therefore takes `n * 60 ms` (e.g. 12 frames -> 0.72 s/cell).
pub const ORIGINAL_FRAME_MS: f64 = 60.0;
pub const LOGIC_TICK_HZ: f64 = 1000.0 / ORIGINAL_FRAME_MS;
/// Fallback walk-cycle length (frames) used when an actor's own sprite cycle
/// cannot be resolved (`Res::action_cycle_frames`). 8 frames at ~16.67 Hz
/// ~= 2.1 cells/s.
pub const FALLBACK_WALK_FRAMES: u8 = 8;
/// Wander search radius used by `FUN_00413a84` (`_DAT_0081d6e0`/`_DAT_0081d6a4`).
/// Set statically by `FUN_0042781c` (0x42781C): the 800x600 profile uses
/// `x = 20`, `y = 9` (the 640x480 profile uses `17`/`7`). `FUN_00413a84` samples
/// `rng() % (2*radius)`, and the accepted candidate must also lie strictly
/// inside the group rectangle, so these bound the random offset.
pub const WANDER_RADIUS_X: i32 = 20;
pub const WANDER_RADIUS_Y: i32 = 9;
/// `FUN_00413a84` retry count.
pub const WANDER_MAX_ATTEMPTS: u8 = 10;
/// `Entity+0x02` bit set on wandering citizens (`FUN_004168a4` dispatch gate).
pub const FLAG_WANDER: u8 = 0x40;
