//! Original world-map / travel screen (interface mode 5).
//!
//! Recovered from `konung.res`:
//! * `FUN_0042CBD0(4)` sets mode 5 and requests the map composite; the frame
//!   presenter then calls `FUN_00428390`.
//! * `FUN_00428390` tiles the `INTR[5]` stone background over the viewport,
//!   blits the `INTR[4]` parchment at the viewport origin, then blits the
//!   `INTR[179]` (502x361) map overlay only through discovered runs.
//! * Discovered mask: 56 bytes (`0x81CF80`), indexed by the run value.
//! * Click hit-test: `FUN_004381C8` mode 5 + `FUN_00438900` over
//!   `0x456A1C`/`0x450EBC`; the result is `(location << 8) | 0x103`.
//!
//! [`draw`] covers the interface viewport only, so the left party panel and the
//! bottom bar keep showing the HUD underneath. The discovered overlay is
//! composed once and cached by [`mask_hash`], because the run table is 361 rows
//! and the mask changes only when a map item reveals a location.

use crate::hud::layout;
use crate::res::{blit, blit_sub, Image, Img, Res};

pub mod data;

pub use data::{MAP_H, MAP_RESERVED, MAP_ROWS, MAP_W};

/// Map overlay offset relative to the interface viewport.
pub fn overlay_offset(logical_w: i32) -> (i32, i32) {
    if logical_w <= 640 {
        (13, 13)
    } else {
        (80, 60)
    }
}

/// Exact `FUN_00438900` lookup over the map run table. Returns the location
/// index for map-local `(x, y)`.
pub fn location_at(x: i32, y: i32) -> Option<u8> {
    if x < 0 || y < 0 || y as usize >= MAP_H {
        return None;
    }
    let runs = MAP_ROWS[y as usize];
    let mut acc = 0i32;
    let mut last = 0u8;
    for &(loc, width) in runs {
        last = loc;
        acc += width as i32;
        if width == 0 || x < acc {
            return Some(loc);
        }
    }
    Some(last)
}

/// First map-local pixel belonging to a location run (diagnostics/tests).
pub fn point_for_location(loc: u8) -> Option<(i32, i32)> {
    for (y, runs) in MAP_ROWS.iter().enumerate() {
        let runs = *runs;
        let mut x = 0i32;
        for &(value, width) in runs {
            if width == 0 {
                break;
            }
            if value == loc {
                return Some((x, y as i32));
            }
            x += width as i32;
        }
    }
    None
}

/// FNV-1a hash of the discovered mask, used to cache the composed overlay.
pub fn mask_hash(discovered: &[u8; 56]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &b in discovered {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Compose the visible parts of `INTR[179]` into a 502x361 RGBA image. Pixels
/// outside every discovered run stay transparent, so the parchment shows
/// through (the original clips each run before blitting the same sprite).
pub fn compose_overlay(res: &mut Res, discovered: &[u8; 56]) -> Option<Img> {
    let src = res.intr_sprite(179)?;
    if src.w < MAP_W || src.h < MAP_H {
        return None;
    }
    let mut data = vec![0u8; MAP_W * MAP_H * 4];
    for row in 0..MAP_H {
        let mut x = 0usize;
        let mut last = 0u8;
        for &(loc, width) in MAP_ROWS[row] {
            last = loc;
            if width == 0 {
                break;
            }
            let w = width as usize;
            let visible = (loc as usize) < 56 && discovered[loc as usize] != 0;
            if visible {
                copy_span(&src, row, x, w, &mut data);
            }
            x = (x + w).min(MAP_W);
        }
        if x < MAP_W && (last as usize) < 56 && discovered[last as usize] != 0 {
            copy_span(&src, row, x, MAP_W - x, &mut data);
        }
    }
    Some(std::rc::Rc::new(Image {
        w: MAP_W,
        h: MAP_H,
        data,
    }))
}

/// Copy one row of `INTR[179]` into the composite buffer.
///
/// `x`/`w` are map-local pixel coordinates; the copy is clamped to `MAP_W` and
/// to the source width, so a run that overshoots the sprite edge is truncated
/// instead of panicking.
fn copy_span(src: &Image, row: usize, x: usize, w: usize, dst: &mut [u8]) {
    let w = w.min(MAP_W.saturating_sub(x)).min(src.w.saturating_sub(x));
    if w == 0 || row >= src.h {
        return;
    }
    let s = (row * src.w + x) * 4;
    let d = (row * MAP_W + x) * 4;
    dst[d..d + w * 4].copy_from_slice(&src.data[s..s + w * 4]);
}

/// Tile the original `INTR[5]` stone background across an interface viewport
/// (`FUN_00430430`).
pub fn draw_stone(frame: &mut [u8], fw: usize, fh: usize, res: &mut Res, vp: crate::hud::Rect) {
    if let Some(tile) = res.intr_sprite(5) {
        let mut ty = 0;
        while ty < vp.h {
            let mut tx = 0;
            while tx < vp.w {
                let w = (tile.w as i32).min(vp.w - tx);
                let h = (tile.h as i32).min(vp.h - ty);
                if w > 0 && h > 0 {
                    blit_sub(
                        frame,
                        fw,
                        fh,
                        &tile.data,
                        tile.w,
                        tile.h,
                        0,
                        0,
                        w as usize,
                        h as usize,
                        vp.x + tx,
                        vp.y + ty,
                    );
                }
                tx += tile.w as i32;
            }
            ty += tile.h as i32;
        }
    }
}

/// Draw the map screen over the already-drawn HUD. Only the interface viewport
/// is covered; the original leaves the left party panel and bottom bar alone.
pub fn draw(
    frame: &mut [u8],
    fw: usize,
    fh: usize,
    res: &mut Res,
    logical_w: i32,
    discovered: &[u8; 56],
    cache: &mut Option<(u64, Img)>,
) {
    let l = layout(logical_w);
    let vp = l.viewport;

    // INTR[5] stone tile (100x100) over the whole viewport.
    draw_stone(frame, fw, fh, res, vp);

    // INTR[4] parchment at the viewport origin, clipped to the viewport.
    if let Some(p) = res.intr_sprite(4) {
        let w = (p.w as i32).min(vp.w);
        let h = (p.h as i32).min(vp.h);
        blit_sub(
            frame,
            fw,
            fh,
            &p.data,
            p.w,
            p.h,
            0,
            0,
            w.max(0) as usize,
            h.max(0) as usize,
            vp.x,
            vp.y,
        );
    }

    // Discovered map overlay.
    let key = mask_hash(discovered);
    let stale = !matches!(cache, Some((k, _)) if *k == key);
    if stale {
        *cache = compose_overlay(res, discovered).map(|img| (key, img));
    }
    if let Some((_, img)) = cache.as_ref() {
        let (dx, dy) = overlay_offset(logical_w);
        blit(frame, fw, fh, &img.data, img.w, img.h, vp.x + dx, vp.y + dy);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every run row terminates with a zero width and never exceeds the 502 px
    /// overlay width.
    #[test]
    fn run_table_is_361_rows_of_502px_reach() {
        assert_eq!(MAP_ROWS.len(), MAP_H);
        for (row, runs) in MAP_ROWS.iter().enumerate() {
            assert!(runs.iter().any(|&(_, w)| w == 0), "row {row} terminator");
            assert!(runs.iter().map(|&(_, w)| w as usize).sum::<usize>() <= MAP_W);
        }
    }

    /// The run walk returns the location covering a pixel, including its last
    /// pixel and the last row, and rejects out-of-range points.
    #[test]
    fn location_lookup_matches_first_run() {
        // Row 0: location 28 for 84 px, location 31 for 91 px, ...
        assert_eq!(location_at(0, 0), Some(28));
        assert_eq!(location_at(83, 0), Some(28));
        assert_eq!(location_at(84, 0), Some(31));
        assert_eq!(location_at(501, 0), Some(38));
        assert_eq!(location_at(0, 360), Some(2));
        assert_eq!(location_at(-1, 0), None);
        assert_eq!(location_at(0, 361), None);
    }

    /// The reserved table marks exactly 14 locations, the `41..=54` block.
    #[test]
    fn reserved_table_has_fourteen_locked_entries() {
        assert_eq!(MAP_RESERVED.iter().filter(|&&b| b != 0).count(), 14);
        assert!(MAP_RESERVED[41..=54].iter().all(|&b| b == 1));
    }

    /// Revealing one location changes the cache key, so the overlay recomposes.
    #[test]
    fn mask_hash_changes_with_discovery() {
        let mut a = [0u8; 56];
        let b = a;
        a[3] = 1;
        assert_ne!(mask_hash(&a), mask_hash(&b));
    }
}
