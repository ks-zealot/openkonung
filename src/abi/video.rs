//! `video` subsystem of [`crate`]: browser movie playback handshake.
//!
//! The Rust side only queues the original clip and post-movie state. The shell
//! resolves the MP4 (packed or manifest fallback), plays it with a normal
//! `<video>` element, and calls [`video_finish`] on end/error/skip. While a clip
//! is pending [`step`] and the input exports are frozen, so the
//! game clock does not run under the overlay.
//!
//! The four triggers follow `FUN_0042ADE4`: startup state 9 plays `POSTER.FLM`;
//! the leader game-over flag plays `GAMEOVER.FLM`; a settlement defeat plays
//! `DEFEAT<location>.FLM`; and collecting WorldObject source index 0 plays
//! `WIN.FLM`. A missing or undecodable movie still applies the post-state,
//! exactly like a failed `PlayAVI`.

use super::*;
use crate::video::{self, VideoAfter, VideoClip};

/// Scratch buffer for the current clip file name (UTF-8, no NUL needed).
static mut VIDEO_NAME_BUF: [u8; 32] = [0; 32];

/// Copy the pending clip's file name into the scratch buffer and return it.
///
/// Returns `None` when no clip is pending or the name exceeds the 32-byte
/// buffer (the longest shipped name is `defeat<loc>.mp4`, well under 32).
fn current_name() -> Option<&'static [u8]> {
    let p = video::pending()?;
    let name = p.clip.file_name();
    let bytes = name.as_bytes();
    if bytes.len() > 32 {
        return None;
    }
    unsafe {
        let buf = &mut *std::ptr::addr_of_mut!(VIDEO_NAME_BUF);
        buf[..bytes.len()].copy_from_slice(bytes);
        Some(&buf[..bytes.len()])
    }
}

/// Queue the startup `POSTER.FLM` intro once per runtime. Returns 1 when a
/// movie was queued, 0 when already played / no frontend exists.
#[no_mangle]
pub extern "C" fn start_intro_video() -> u32 {
    if video::intro_played() || video::is_pending() {
        return 0;
    }
    let has_frontend = runtime_mut().frontend.is_some();
    if !has_frontend {
        return 0;
    }
    video::mark_intro_played();
    video::request(VideoClip::Poster, VideoAfter::Menu { game_active: false }) as u32
}

/// 1 while a movie is waiting to be played/finished.
#[no_mangle]
pub extern "C" fn video_pending() -> u32 {
    video::is_pending() as u32
}

/// Current clip file name (`poster.mp4`, `defeat7.mp4`, ...).
///
/// # Memory contract
///
/// Borrowed UTF-8 from the static scratch buffer; never free it. Null when no
/// clip is pending (check [`video_pending`] first). Pair with
/// [`video_name_len`] - the name is not NUL terminated.
#[no_mangle]
pub extern "C" fn video_name_ptr() -> *const u8 {
    current_name()
        .map(|b| b.as_ptr())
        .unwrap_or(std::ptr::null())
}

/// Byte length of the pending clip name, or 0 when no clip is pending.
#[no_mangle]
pub extern "C" fn video_name_len() -> u32 {
    current_name().map(|b| b.len() as u32).unwrap_or(0)
}

/// Complete or skip the current movie. Always applies the original post-state
/// (a failed decode falls through exactly like a missing `PlayAVI` clip).
///
/// Returns 1 when a pending clip was consumed, 0 when there was nothing to
/// finish.
#[no_mangle]
pub extern "C" fn video_finish() -> u32 {
    let Some(done) = video::finish() else {
        return 0;
    };
    apply_after(done.after);
    1
}

/// Apply the state the original enters after a movie ends.
///
/// `Gameplay` reactivates the preserved live game; `Menu` reactivates (building
/// if needed) the frontend main menu, and when `game_active` is false it disposes
/// the finished run and returns its raw buffers to the loader so a New Game or
/// Load can rebuild the world.
fn apply_after(after: VideoAfter) {
    match after {
        VideoAfter::Gameplay => {
            let rt = runtime_mut();
            if rt.game.is_some() {
                rt.active = ActiveMode::Game;
            }
        }
        VideoAfter::Menu { game_active } => {
            let (host_w, host_h) = {
                let rt = runtime_mut();
                (rt.host_w, rt.host_h)
            };
            let rt = runtime_mut();
            if rt.frontend.is_none() {
                match build_frontend(FrontendRes::R800) {
                    Ok(mut f) => {
                        f.game_active = game_active;
                        apply_host(&mut f, host_w, host_h);
                        rt.frontend = Some(f);
                    }
                    Err(e) => eprintln!("video menu transition: {e}"),
                }
            }
            if let Some(f) = rt.frontend.as_mut() {
                f.game_active = game_active;
                f.screen = FrontendScreen::MainMenu;
                f.hovered = None;
                f.save_slot_hover = None;
            }
            rt.active = ActiveMode::Frontend;
            if !game_active {
                // Game over / victory end the current run; the original hides
                // Resume via `_81D5F8 = 0`. Return the raw resource buffers to
                // the loader so New Game / Load can rebuild the world.
                rt.dispose_game();
            }
        }
    }
}

// --------------------------------------------------------------------------
// Diagnostics
// --------------------------------------------------------------------------

/// Same as [`video_pending`] (diagnostic alias used by `tools/video_smoke.mjs`).
#[no_mangle]
pub extern "C" fn debug_video_pending() -> u32 {
    video_pending()
}

/// Same as [`video_name_len`] (diagnostic alias).
#[no_mangle]
pub extern "C" fn debug_video_name_len() -> u32 {
    video_name_len()
}

/// Same as [`video_name_ptr`] (diagnostic alias).
///
/// # Memory contract
///
/// See [`video_name_ptr`]: borrowed static buffer, null when nothing is pending.
#[no_mangle]
pub extern "C" fn debug_video_name_ptr() -> *const u8 {
    video_name_ptr()
}

/// Diagnostic: forget the pending clip and the "intro already played" flag.
#[no_mangle]
pub extern "C" fn debug_video_reset() {
    video::reset();
}

/// Queue a clip by selector for smoke tests: 0 poster, 1 win, 2 gameover,
/// 3 defeat (`loc`), 4 credits. `after` 0 = menu without resume, 1 = gameplay.
///
/// Returns 1 when the clip was queued, 0 for an unknown selector or when another
/// clip is already pending.
#[no_mangle]
pub extern "C" fn debug_queue_video(kind: u32, loc: u32, after: u32) -> u32 {
    let clip = match kind {
        0 => VideoClip::Poster,
        1 => VideoClip::Win,
        2 => VideoClip::GameOver,
        3 => VideoClip::Defeat(loc as u8),
        4 => VideoClip::Credits,
        _ => return 0,
    };
    let after = if after == 1 {
        VideoAfter::Gameplay
    } else {
        VideoAfter::Menu { game_active: false }
    };
    video::request(clip, after) as u32
}
