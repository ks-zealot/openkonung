//! `raster` subsystem of [`crate::res`] (split from `res.rs`).

use super::*;

#[derive(Clone)]
pub struct Image {
    pub w: usize,
    pub h: usize,
    pub data: Vec<u8>, // RGBA
}


pub type Img = Rc<Image>;


pub fn palette_rgb(graph: &[u8], palette_id: usize) -> [[u8; 3]; 256] {
    let mut out = [[0u8; 3]; 256];
    let base = palette_id * 512;
    if base + 512 > graph.len() {
        return out;
    }
    for i in 0..256 {
        let v = u16(graph, base + i * 2);
        let b = (v & 31) * 255 / 31;
        let g = ((v >> 5) & 31) * 255 / 31;
        let r = ((v >> 10) & 31) * 255 / 31;
        out[i] = [r as u8, g as u8, b as u8];
    }
    out
}


/// Decode the Konung indexed-image grammar into index + alpha arrays.
pub fn decode_indexed_rle(payload: &[u8], off: usize) -> (Vec<u8>, Vec<u8>, usize, usize) {
    if off + 4 > payload.len() {
        return (Vec::new(), Vec::new(), 0, 0);
    }
    let w = u16(payload, off) as usize;
    let h = u16(payload, off + 2) as usize;
    if w == 0 || h == 0 || w > 4096 || h > 4096 {
        return (Vec::new(), Vec::new(), 0, 0);
    }
    let mut idx = vec![0u8; w * h];
    let mut alpha = vec![0u8; w * h];
    // Row sizes live in a table immediately after the header; the row payloads
    // themselves are concatenated with no inline size prefix.
    let sizes_at = off + 4;
    if sizes_at + 2 * h > payload.len() {
        return (Vec::new(), Vec::new(), 0, 0);
    }
    let mut p = off + 4 + 2 * h;
    for y in 0..h {
        let row_size = u16(payload, sizes_at + y * 2) as usize;
        if p + row_size > payload.len() {
            break;
        }
        let row = &payload[p..p + row_size];
        p += row_size;
        let mut x = 0usize;
        let mut q = 0usize;
        while q < row.len() {
            let cmd = row[q];
            q += 1;
            if cmd == 0 {
                break;
            }
            if cmd & 0x80 != 0 {
                x += (cmd & 0x7F) as usize;
            } else {
                let n = cmd as usize;
                if q + n > row.len() {
                    break;
                }
                let mut take = n;
                if x + take > w {
                    take = w.saturating_sub(x);
                }
                for k in 0..take {
                    let di = y * w + x + k;
                    idx[di] = row[q + k];
                    alpha[di] = 255;
                }
                q += n;
                x += n;
            }
        }
    }
    (idx, alpha, w, h)
}


pub fn indexed_to_rgba(idx: &[u8], alpha: &[u8], w: usize, h: usize, pal: &[[u8; 3]; 256]) -> Image {
    let mut data = vec![0u8; w * h * 4];
    for i in 0..w * h {
        if alpha[i] > 0 {
            let c = pal[idx[i] as usize];
            data[i * 4] = c[0];
            data[i * 4 + 1] = c[1];
            data[i * 4 + 2] = c[2];
            data[i * 4 + 3] = 255;
        }
    }
    Image { w, h, data }
}


pub fn u16(b: &[u8], o: usize) -> u32 {
    (b[o] as u32) | ((b[o + 1] as u32) << 8)
}

pub fn i16(b: &[u8], o: usize) -> i32 {
    u16(b, o) as u16 as i16 as i32
}

pub fn u32(b: &[u8], o: usize) -> u32 {
    (b[o] as u32)
        | ((b[o + 1] as u32) << 8)
        | ((b[o + 2] as u32) << 16)
        | ((b[o + 3] as u32) << 24)
}

pub fn i32v(b: &[u8], o: usize) -> i32 {
    u32(b, o) as i32
}


pub fn cell_to_screen(cx: i32, cy: i32) -> (i32, i32) {
    (
        MAP_ORIGIN_X + cx * GRID_X_STEP - (cy & 1) * GRID_ODD_SHIFT,
        cy * GRID_Y_STEP,
    )
}


/// Alpha-composite `src` onto `dst` at (ox, oy). Straight "over" operator.
pub fn blit(dst: &mut [u8], dw: usize, dh: usize, src: &[u8], sw: usize, sh: usize, ox: i32, oy: i32) {
    if sw == 0 || sh == 0 {
        return;
    }
    for sy in 0..sh {
        let dy = oy + sy as i32;
        if dy < 0 || dy >= dh as i32 {
            continue;
        }
        let srow = sy * sw * 4;
        let drow = dy as usize * dw * 4;
        for sx in 0..sw {
            let dx = ox + sx as i32;
            if dx < 0 || dx >= dw as i32 {
                continue;
            }
            let si = srow + sx * 4;
            let a = src[si + 3];
            if a == 0 {
                continue;
            }
            let di = drow + dx as usize * 4;
            if a == 255 {
                dst[di..di + 4].copy_from_slice(&src[si..si + 4]);
            } else {
                let sa = a as u32;
                let inv = 255 - sa;
                dst[di] = ((src[si] as u32 * sa + dst[di] as u32 * inv) / 255) as u8;
                dst[di + 1] = ((src[si + 1] as u32 * sa + dst[di + 1] as u32 * inv) / 255) as u8;
                dst[di + 2] = ((src[si + 2] as u32 * sa + dst[di + 2] as u32 * inv) / 255) as u8;
                dst[di + 3] = 255;
            }
        }
    }
}


/// Alpha-composite only source rows `[sy0, sh)` of `src` onto `dst`, mapping
/// source row `sy` to destination row `oy + sy`. Used by the party-panel HP
/// indicator, whose fill is the bottom `h` rows of the full-height bar sprite
/// (`FUN_0042C730`: `clip_y1 = origin_y + H - h`).
#[allow(clippy::too_many_arguments)]
pub fn blit_rows(
    dst: &mut [u8],
    dw: usize,
    dh: usize,
    src: &[u8],
    sw: usize,
    sh: usize,
    sy0: usize,
    ox: i32,
    oy: i32,
) {
    if sw == 0 || sh == 0 || sy0 >= sh {
        return;
    }
    for sy in sy0..sh {
        let dy = oy + sy as i32;
        if dy < 0 || dy >= dh as i32 {
            continue;
        }
        let srow = sy * sw * 4;
        let drow = dy as usize * dw * 4;
        for sx in 0..sw {
            let dx = ox + sx as i32;
            if dx < 0 || dx >= dw as i32 {
                continue;
            }
            let si = srow + sx * 4;
            let a = src[si + 3];
            if a == 0 {
                continue;
            }
            let di = drow + dx as usize * 4;
            if a == 255 {
                dst[di..di + 4].copy_from_slice(&src[si..si + 4]);
            } else {
                let sa = a as u32;
                let inv = 255 - sa;
                dst[di] = ((src[si] as u32 * sa + dst[di] as u32 * inv) / 255) as u8;
                dst[di + 1] = ((src[si + 1] as u32 * sa + dst[di + 1] as u32 * inv) / 255) as u8;
                dst[di + 2] = ((src[si + 2] as u32 * sa + dst[di + 2] as u32 * inv) / 255) as u8;
                dst[di + 3] = 255;
            }
        }
    }
}


/// Fill a rectangle (opaque) clipping to the buffer.
pub fn fill_rect(dst: &mut [u8], dw: usize, dh: usize, x: i32, y: i32, w: i32, h: i32, c: [u8; 4]) {
    let x0 = x.max(0);
    let y0 = y.max(0);
    let x1 = (x + w).min(dw as i32);
    let y1 = (y + h).min(dh as i32);
    for yy in y0..y1 {
        let row = yy as usize * dw * 4;
        for xx in x0..x1 {
            let di = row + xx as usize * 4;
            dst[di..di + 4].copy_from_slice(&c);
        }
    }
}


/// Draw a filled ellipse using a simple scanline test.
pub fn fill_ellipse(dst: &mut [u8], dw: usize, dh: usize, cx: f32, cy: f32, rx: f32, ry: f32, c: [u8; 4]) {
    if rx <= 0.0 || ry <= 0.0 {
        return;
    }
    let y0 = (cy - ry).floor().max(0.0) as i32;
    let y1 = (cy + ry).ceil().min(dh as f32 - 1.0) as i32;
    for yy in y0..=y1 {
        let dy = (yy as f32 + 0.5 - cy) / ry;
        if dy.abs() > 1.0 {
            continue;
        }
        let half = rx * (1.0 - dy * dy).max(0.0).sqrt();
        let sx = (cx - half).floor() as i32;
        let ex = (cx + half).ceil() as i32;
        for xx in sx.max(0)..=ex.min(dw as i32 - 1) {
            let di = yy as usize * dw * 4 + xx as usize * 4;
            dst[di..di + 4].copy_from_slice(&c);
        }
    }
}


/// Bresenham line.
pub fn line(dst: &mut [u8], dw: usize, dh: usize, mut x0: i32, mut y0: i32, x1: i32, y1: i32, c: [u8; 4]) {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        if x0 >= 0 && x0 < dw as i32 && y0 >= 0 && y0 < dh as i32 {
            let di = y0 as usize * dw * 4 + x0 as usize * 4;
            dst[di..di + 4].copy_from_slice(&c);
        }
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}


