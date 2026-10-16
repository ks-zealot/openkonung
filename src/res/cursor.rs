//! `cursor` subsystem of [`crate::res`]: the original 11 game mouse cursors.
//!
//! The original hides the Win32 cursor (`ShowCursor(0)`; WndProc `SetCursor(0)`)
//! and draws its own each frame (`FUN_0042A470` -> `FUN_0043D2FE` at the pointer
//! globals `_DAT_0081D5C0/_D5C4`), selecting the frame with `FUN_00428740`
//! (hover context) over the base id `_DAT_0081D660` (`FUN_00424E0C` maps the
//! active item/action class `0..=0xC` to `{10,2,0,8,9}`).
//!
//! The 11 `0x20 x 0x20` bitmaps are the first blocks of `GRAPH.RES` (loader
//! `FUN_004392B4` copies them into the RAM table `0x81A83C`; `res/graph.rs`
//! already steps over the same 11 blocks). Each block is
//! `[u32 type=0x05000000][u32 size][sprite]`, and the sprite is 16-bit color:
//! `[u16 w][u16 h][h * u16 row-size][row streams]`, where `0x00` ends a row,
//! `b & 0x80` skips `(b & 0x7F)` pixels, and any other `b` copies `b` literal
//! pixels of 2 bytes (`FUN_0043D2FE`).

use super::*;

/// `GRAPH.RES` offset of the 11 cursor blocks.
pub const CURSOR_GRAPH_OFF: usize = 0x20000;
/// Number of original cursor frames (`FUN_004392B4` copies 11 into `0x81A83C`).
pub const CURSOR_COUNT: usize = 11;
/// Original cursor width (`FUN_0042A470` blits `0x20`).
pub const CURSOR_W: usize = 0x20;
/// Original cursor height (`FUN_0042A470` blits `0x20`).
pub const CURSOR_H: usize = 0x20;

/// Sprite payload offset of cursor block `i` inside `graph`, or `None` when the
/// archive is truncated.
fn cursor_block(graph: &[u8], i: usize) -> Option<usize> {
    if i >= CURSOR_COUNT {
        return None;
    }
    let mut pos = CURSOR_GRAPH_OFF;
    for k in 0..=i {
        if pos + 8 > graph.len() {
            return None;
        }
        let size = u32(graph, pos + 4) as usize;
        let payload = pos + 8;
        if payload + size > graph.len() {
            return None;
        }
        if k == i {
            return Some(payload);
        }
        pos = payload + size;
    }
    None
}

/// Decode cursor frame `i` (`0..CURSOR_COUNT`) from `GRAPH.RES` to RGBA.
/// Transparent pixels keep alpha `0`.
pub fn cursor_image(graph: &[u8], i: usize) -> Option<Img> {
    let pay = cursor_block(graph, i)?;
    let p = &graph[pay..];
    if p.len() < 4 {
        return None;
    }
    let w = u16(p, 0) as usize;
    let h = u16(p, 2) as usize;
    if w == 0 || h == 0 || w > 256 || h > 256 {
        return None;
    }
    if 4 + 2 * h > p.len() {
        return None;
    }
    let mut data = vec![0u8; w * h * 4];
    let mut q = 4 + 2 * h;
    for y in 0..h {
        let rs = u16(p, 4 + y * 2) as usize;
        if q + rs > p.len() {
            break;
        }
        let row = &p[q..q + rs];
        q += rs;
        let mut x = 0usize;
        let mut rq = 0usize;
        while rq < row.len() {
            let b = row[rq];
            rq += 1;
            if b == 0 {
                break;
            }
            if b & 0x80 != 0 {
                x += (b & 0x7f) as usize;
            } else {
                for _ in 0..b as usize {
                    if x >= w || rq + 2 > row.len() {
                        break;
                    }
                    let v = row[rq] as u32 | ((row[rq + 1] as u32) << 8);
                    rq += 2;
                    let r = (((v >> 10) & 31) * 255 / 31) as u8;
                    let g = (((v >> 5) & 31) * 255 / 31) as u8;
                    let bl = ((v & 31) * 255 / 31) as u8;
                    let di = (y * w + x) * 4;
                    data[di] = r;
                    data[di + 1] = g;
                    data[di + 2] = bl;
                    data[di + 3] = 255;
                    x += 1;
                }
            }
        }
    }
    Some(Rc::new(Image { w, h, data }))
}

impl Res {
    /// Number of original cursor frames (11).
    pub fn cursor_count(&self) -> usize {
        CURSOR_COUNT
    }

    /// The original mouse cursor frame `i` (`0..11`), decoded from `GRAPH.RES`.
    /// `None` when the archive is absent/truncated or `i` is out of range.
    pub fn cursor_sprite(&self, i: usize) -> Option<Img> {
        cursor_image(&self.graph, i)
    }
}
