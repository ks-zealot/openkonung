//! Original movie requests and the browser-side playback handshake.
//!
//! The shipped `*.flm` files are AVI/Indeo-5; browsers cannot decode them, so
//! `tools/transcode_videos.py` produces `www/video/<clip>.mp4`. This module is
//! the small, testable state machine between a game/frontend trigger and the
//! shell: the game requests a clip and a post-video outcome, JS plays it, then
//! calls `video_finish`.
//!
//! Original references: `FUN_0042ADE4` (`PlayAVI.dll::playAVIFile`) builds
//! `VIDEO\<name>`, blocks, and on completion performs
//! `_81D5AC = _81D6B8`. The remake keeps that ordering but makes it async.
//!
//! The pending request is a single thread-local slot: [`request`] fails while a
//! movie is queued (the original call blocks, so a second trigger cannot
//! interleave), [`finish`] takes the slot, and `abi/video.rs` then applies the
//! [`VideoAfter`] outcome.

use std::cell::{Cell, RefCell};

/// One original movie, mapped to its lowercase generated MP4 stem.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoClip {
    /// `POSTER.FLM`, the startup movie (original startup state 9).
    Poster,
    /// `WIN.FLM`, played when WorldObject source index 0 is collected.
    Win,
    /// `GAMEOVER.FLM`, played when the leader's game-over flag is set.
    GameOver,
    /// `DEFEAT<location>.FLM`, played when a settlement defence is lost.
    Defeat(
        /// Location id of the defeated settlement (`DEFEAT<loc>.FLM`).
        u8,
    ),
    /// `credits.flm`, played by the launcher only - the game never requests it.
    Credits,
}

impl VideoClip {
    /// Lowercase stem used by the manifest / pack video map.
    pub fn key(self) -> String {
        match self {
            VideoClip::Poster => "poster".to_string(),
            VideoClip::Win => "win".to_string(),
            VideoClip::GameOver => "gameover".to_string(),
            VideoClip::Defeat(loc) => format!("defeat{loc}"),
            VideoClip::Credits => "credits".to_string(),
        }
    }

    /// Generated MP4 file name (`<key>.mp4`) that the shell loads.
    pub fn file_name(self) -> String {
        format!("{}.mp4", self.key())
    }

    /// Original source name, useful for diagnostics/docs.
    pub fn source_name(self) -> String {
        format!("{}.FLM", self.key().to_ascii_uppercase())
    }

    /// Parse a manifest/pack video key back into a clip.
    ///
    /// Accepts the lowercase stem with or without `.mp4`; the original `.FLM`
    /// spelling is deliberately rejected because it is not a manifest key.
    pub fn parse(name: &str) -> Option<VideoClip> {
        let stem = name
            .strip_suffix(".mp4")
            .unwrap_or(name)
            .to_ascii_lowercase();
        match stem.as_str() {
            "poster" => Some(VideoClip::Poster),
            "win" => Some(VideoClip::Win),
            "gameover" => Some(VideoClip::GameOver),
            "credits" => Some(VideoClip::Credits),
            other => other
                .strip_prefix("defeat")
                .and_then(|n| n.parse::<u8>().ok())
                .map(VideoClip::Defeat),
        }
    }
}

/// What the runtime should do when the movie ends or is skipped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoAfter {
    /// Return to the original menu; `game_active` controls Resume visibility.
    Menu {
        /// Whether a live game is still in memory, i.e. whether Resume exists.
        game_active: bool,
    },
    /// Resume gameplay (`_81D6B8 = 0` in the defeat path).
    Gameplay,
}

/// One queued movie plus the outcome to apply when it ends or is skipped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PendingVideo {
    /// Clip the shell should play.
    pub clip: VideoClip,
    /// Post-video state to apply through `abi/video.rs`.
    pub after: VideoAfter,
}

thread_local! {
    static PENDING: RefCell<Option<PendingVideo>> = const { RefCell::new(None) };
    static INTRO_PLAYED: Cell<bool> = const { Cell::new(false) };
}

/// Queue one movie. Only one can be pending; a trigger while another movie is
/// pending is ignored (the original player is blocking).
pub fn request(clip: VideoClip, after: VideoAfter) -> bool {
    PENDING.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_some() {
            return false;
        }
        *slot = Some(PendingVideo { clip, after });
        true
    })
}

/// The currently queued movie, if any.
pub fn pending() -> Option<PendingVideo> {
    PENDING.with(|slot| *slot.borrow())
}

/// Whether a movie is queued; a trigger while one is pending is ignored.
pub fn is_pending() -> bool {
    pending().is_some()
}

/// Complete or skip the current movie and return its post-state.
pub fn finish() -> Option<PendingVideo> {
    PENDING.with(|slot| slot.borrow_mut().take())
}

/// Whether the startup poster has already been played in this session.
pub fn intro_played() -> bool {
    INTRO_PLAYED.with(|v| v.get())
}

/// Remember that the startup poster has been played, so it never repeats.
pub fn mark_intro_played() {
    INTRO_PLAYED.with(|v| v.set(true));
}

/// Test/debug reset (also allows replaying the intro in one wasm session).
pub fn reset() {
    PENDING.with(|slot| *slot.borrow_mut() = None);
    INTRO_PLAYED.with(|v| v.set(false));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Generated stems, file names and manifest keys agree, and the original
    /// `.FLM` spelling is not a valid manifest key.
    #[test]
    fn clip_names_match_the_generated_assets() {
        assert_eq!(VideoClip::Poster.file_name(), "poster.mp4");
        assert_eq!(VideoClip::Defeat(7).file_name(), "defeat7.mp4");
        assert_eq!(VideoClip::GameOver.source_name(), "GAMEOVER.FLM");
        assert_eq!(
            VideoClip::parse("defeat19.mp4"),
            Some(VideoClip::Defeat(19))
        );
        assert_eq!(VideoClip::parse("POSTER.FLM"), None);
    }

    /// A second request while a movie is pending is refused, and `finish`
    /// consumes the slot so the next request is accepted.
    #[test]
    fn only_one_movie_is_pending_at_a_time() {
        reset();
        assert!(request(
            VideoClip::Poster,
            VideoAfter::Menu { game_active: false }
        ));
        assert!(!request(
            VideoClip::Win,
            VideoAfter::Menu { game_active: false }
        ));
        assert_eq!(pending().unwrap().clip, VideoClip::Poster);
        assert_eq!(finish().unwrap().clip, VideoClip::Poster);
        assert!(pending().is_none());
        reset();
    }
}
