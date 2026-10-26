//! `video` subsystem of [`crate::lib`]: browser movie playback handshake.
//!
//! The Rust side only queues the original clip and post-movie state. The shell
//! resolves the MP4 (packed or manifest fallback), plays it with a normal
//! `<video>` element, and calls [`video_finish`] on end/error/skip.

use super::*;
use crate::video::{self, VideoAfter, VideoClip};

static mut VIDEO_NAME_BUF: [u8; 32] = [0; 32];

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
#[no_mangle]
pub extern "C" fn video_name_ptr() -> *const u8 {
    current_name()
        .map(|b| b.as_ptr())
        .unwrap_or(std::ptr::null())
}

#[no_mangle]
pub extern "C" fn video_name_len() -> u32 {
    current_name().map(|b| b.len() as u32).unwrap_or(0)
}

/// Complete or skip the current movie. Always applies the original post-state
/// (a failed decode falls through exactly like a missing `PlayAVI` clip).
#[no_mangle]
pub extern "C" fn video_finish() -> u32 {
    let Some(done) = video::finish() else {
        return 0;
    };
    apply_after(done.after);
    1
}

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
                // Resume via `_81D5F8 = 0`.
                rt.game = None;
            }
        }
    }
}

// --------------------------------------------------------------------------
// Diagnostics
// --------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn debug_video_pending() -> u32 {
    video_pending()
}

#[no_mangle]
pub extern "C" fn debug_video_name_len() -> u32 {
    video_name_len()
}

#[no_mangle]
pub extern "C" fn debug_video_name_ptr() -> *const u8 {
    video_name_ptr()
}

#[no_mangle]
pub extern "C" fn debug_video_reset() {
    video::reset();
}

/// Queue a clip by selector for smoke tests: 0 poster, 1 win, 2 gameover,
/// 3 defeat (`loc`), 4 credits. `after` 0 = menu without resume, 1 = gameplay.
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
