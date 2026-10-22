//! World rendering.
//!
//! Layer domains (source-of-truth Rev 3, §6/§7/§9/§10) are kept explicit:
//! - base terrain record `(col,row)` → [`terrain_to_local_px`];
//! - Entity/runtime cell → [`crate::coords::entity_to_local_px`];
//! - prop/object grid → [`prop_to_local_px`];
//! - GRAPH placement → [`graph_anchor_to_local_px`] (identity).
//!
//! The playable view is rendered **viewport-culled** directly into the frame
//! (`draw_background` + dynamic sprites), so no full-map RGBA surface is
//! allocated. `render_location_surface` is retained as the native/parity
//! full-surface renderer used by `examples/dump.rs` and `tools/parity.py`.

use std::collections::HashMap;
use std::rc::Rc;

use crate::loc::{
    graph_anchor_to_local_px, prop_to_local_px, terrain_layers, terrain_to_local_px, Location,
    LOCAL_PIXEL_H, LOCAL_PIXEL_W, LOCAL_TERRAIN_H, LOCAL_TERRAIN_W,
};
use crate::res::{blit, Image, Img, Res, TERRAIN_TILE_H, TERRAIN_TILE_W};

pub struct World {
    pub res: Res,
    pub locs: Vec<Location>,
    pub loc_index: HashMap<u32, usize>,
    thumb_cache: HashMap<(u32, i32, i32), Img>,
}

/// Counts of background layer tiles drawn in the last viewport pass.
#[derive(Clone, Copy, Debug, Default)]
pub struct LayerCounts {
    pub terrain_low: usize,
    pub terrain_high: usize,
    pub graph: usize,
}

/// A resolved world sprite (prop or actor) positioned at a local-world anchor.
/// `(gx, gy)` is the resource-side sprite offset; `(lx, ly)` is the projected
/// anchor used for depth sorting.
#[derive(Clone)]
pub struct WorldSprite {
    pub img: Img,
    pub gx: i32,
    pub gy: i32,
    pub lx: f64,
    pub ly: f64,
}

impl World {
    pub fn new(res: Res, locs: Vec<Location>) -> World {
        let loc_index = locs
            .iter()
            .enumerate()
            .map(|(i, l)| (l.loc_id, i))
            .collect();
        World {
            res,
            locs,
            loc_index,
            thumb_cache: HashMap::new(),
        }
    }

    pub fn loc(&self, loc_id: u32) -> Option<&Location> {
        self.loc_index.get(&loc_id).map(|&i| &self.locs[i])
    }

    /// Draw the static background (both terrain layers + GRAPH placements)
    /// directly into `frame`, clipped to the viewport.
    pub fn draw_background(
        &mut self,
        frame: &mut [u8],
        fw: usize,
        fh: usize,
        loc_id: u32,
        ox: i32,
        oy: i32,
    ) -> LayerCounts {
        let idx = match self.loc_index.get(&loc_id) {
            Some(&i) => i,
            None => return LayerCounts::default(),
        };
        draw_background(&mut self.res, &self.locs[idx], frame, fw, fh, ox, oy)
    }

    /// Resolve all object/prop sprites for a location (for the dynamic depth
    /// pass). Props use [`prop_to_local_px`] and are resolved with frame 0.
    pub fn collect_props(&mut self, loc_id: u32) -> Vec<WorldSprite> {
        let idx = match self.loc_index.get(&loc_id) {
            Some(&i) => i,
            None => return Vec::new(),
        };
        let mut out = Vec::with_capacity(self.locs[idx].props.len());
        for p in &self.locs[idx].props {
            let (img, gx, gy) = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.res.prop_sprite(p.object_id, 0, p.palette)
            })) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let (lx, ly) = prop_to_local_px(p.grid_x, p.grid_y);
            out.push(WorldSprite {
                img,
                gx,
                gy,
                lx: lx as f64,
                ly: ly as f64,
            });
        }
        out
    }

    /// Front-most opaque prop sprite under a local-world point. Returns
    /// `(prop_table_index, grid_x, grid_y)`.
    pub fn prop_at_local(&mut self, loc_id: u32, lx: i32, ly: i32) -> Option<(usize, i32, i32)> {
        let idx = *self.loc_index.get(&loc_id)?;
        let loc = &self.locs[idx];
        let props: Vec<(usize, i32, i32)> = loc
            .props
            .iter()
            .map(|p| (p.ri, p.grid_x, p.grid_y))
            .collect();
        let mut best: Option<(usize, i32, i32, i32)> = None;
        for (ri, gx, gy) in props {
            let Some(p) = self.locs[idx].props.iter().find(|p| p.ri == ri) else {
                continue;
            };
            let (img, sx, sy) = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.res.prop_sprite(p.object_id, 0, p.palette)
            })) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let (plx, ply) = prop_to_local_px(gx, gy);
            let px = lx - (plx + sx);
            let py = ly - (ply + sy);
            if px < 0 || py < 0 || px >= img.w as i32 || py >= img.h as i32 {
                continue;
            }
            let a = img.data[((py as usize * img.w + px as usize) * 4) + 3];
            if a == 0 {
                continue;
            }
            let front = (ply + sy) as i32;
            if best.map(|(_, _, _, by)| front >= by).unwrap_or(true) {
                best = Some((ri, gx, gy, front));
            }
        }
        best.map(|(i, x, y, _)| (i, x, y))
    }

    /// `FUN_0043E4EA` equivalent: is the local-world point opaque on prop
    /// sprite `index`?
    pub fn prop_opaque_at(&mut self, loc_id: u32, index: usize, lx: i32, ly: i32) -> bool {
        let idx = match self.loc_index.get(&loc_id) {
            Some(&i) => i,
            None => return false,
        };
        let Some(p) = self.locs[idx].props.iter().find(|p| p.ri == index) else {
            return false;
        };
        let (gx, gy, object_id, palette) = (p.grid_x, p.grid_y, p.object_id, p.palette);
        let (img, sx, sy) = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.res.prop_sprite(object_id, 0, palette)
        })) {
            Ok(v) => v,
            Err(_) => return false,
        };
        let (plx, ply) = prop_to_local_px(gx, gy);
        let px = lx - (plx + sx);
        let py = ly - (ply + sy);
        if px < 0 || py < 0 || px >= img.w as i32 || py >= img.h as i32 {
            return false;
        }
        img.data[((py as usize * img.w + px as usize) * 4) + 3] != 0
    }

    pub fn thumbnail(&mut self, loc_id: u32, w: i32, h: i32) -> Img {
        if let Some(t) = self.thumb_cache.get(&(loc_id, w, h)) {
            return t.clone();
        }
        let img = match self.loc_index.get(&loc_id).map(|&i| &self.locs[i]) {
            Some(loc) => Rc::new(render_thumbnail(&mut self.res, loc, w, h)),
            None => Rc::new(Image::new(
                w.max(1) as usize,
                h.max(1) as usize,
                [12, 15, 14, 255],
            )),
        };
        self.thumb_cache.insert((loc_id, w, h), img.clone());
        img
    }

    pub fn clear_caches(&mut self) {
        self.thumb_cache.clear();
    }
}

fn on_screen(sx: i32, sy: i32, sw: i32, sh: i32, fw: usize, fh: usize) -> bool {
    sx + sw > 0 && sy + sh > 0 && sx < fw as i32 && sy < fh as i32
}

/// Viewport-culled background: base terrain (both byte layers) then GRAPH
/// placements, in their own coordinate domains. Props/actors are drawn by the
/// caller's dynamic depth pass.
pub fn draw_background(
    res: &mut Res,
    loc: &Location,
    frame: &mut [u8],
    fw: usize,
    fh: usize,
    ox: i32,
    oy: i32,
) -> LayerCounts {
    let mut counts = LayerCounts::default();
    let cells = loc.terrain_cells();
    for row in 0..LOCAL_TERRAIN_H {
        for col in 0..LOCAL_TERRAIN_W {
            let idx = (row * LOCAL_TERRAIN_W + col) as usize;
            if idx >= cells.len() {
                continue;
            }
            let (lx, ly) = terrain_to_local_px(col, row);
            let sx = ox + lx;
            let sy = oy + ly;
            if !on_screen(sx, sy, TERRAIN_TILE_W, TERRAIN_TILE_H, fw, fh) {
                continue;
            }
            let (low, high) = terrain_layers(cells[idx]);
            if let Some(r) = low {
                if let Some(tile) = res.terrain_tile(r) {
                    blit(frame, fw, fh, &tile.data, tile.w, tile.h, sx, sy);
                    counts.terrain_low += 1;
                }
            }
            if let Some(r) = high {
                if let Some(tile) = res.terrain_tile(r) {
                    blit(frame, fw, fh, &tile.data, tile.w, tile.h, sx, sy);
                    counts.terrain_high += 1;
                }
            }
        }
    }

    for g in &loc.graph_placements {
        let (lx, ly) = graph_anchor_to_local_px(g.x as i32, g.y as i32);
        let sx = ox + lx;
        let sy = oy + ly;
        if !on_screen(sx, sy, TERRAIN_TILE_W, TERRAIN_TILE_H, fw, fh) {
            continue;
        }
        if let Some(tile) = res.terrain_tile(g.tile as usize) {
            blit(frame, fw, fh, &tile.data, tile.w, tile.h, sx, sy);
            counts.graph += 1;
        }
    }
    counts
}

/// Nearest-neighbour scaled blit for thumbnails (cheap, no per-tile allocation).
fn blit_scaled_nearest(
    dst: &mut [u8],
    dw: usize,
    dh: usize,
    src: &[u8],
    sw: usize,
    sh: usize,
    dx: i32,
    dy: i32,
    dtw: i32,
    dth: i32,
) {
    if sw == 0 || sh == 0 || dtw <= 0 || dth <= 0 {
        return;
    }
    for ty in 0..dth {
        let dyv = dy + ty;
        if dyv < 0 || dyv >= dh as i32 {
            continue;
        }
        let syp = (((ty as f32 + 0.5) * sh as f32 / dth as f32) as usize).min(sh - 1);
        for tx in 0..dtw {
            let dxv = dx + tx;
            if dxv < 0 || dxv >= dw as i32 {
                continue;
            }
            let sxp = (((tx as f32 + 0.5) * sw as f32 / dtw as f32) as usize).min(sw - 1);
            let si = (syp * sw + sxp) * 4;
            let a = src[si + 3];
            if a == 0 {
                continue;
            }
            let di = dyv as usize * dw * 4 + dxv as usize * 4;
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

/// Build a small thumbnail by sampling the corrected geometry directly, without
/// allocating a full-resolution surface.
pub fn render_thumbnail(res: &mut Res, loc: &Location, w: i32, h: i32) -> Image {
    let w = w.max(1) as usize;
    let h = h.max(1) as usize;
    let mut out = Image::new(w, h, [12, 15, 14, 255]);
    let scale = (w as f32 / LOCAL_PIXEL_W as f32).min(h as f32 / LOCAL_PIXEL_H as f32);
    if scale <= 0.0 {
        return out;
    }
    let nw = (LOCAL_PIXEL_W as f32 * scale) as i32;
    let nh = (LOCAL_PIXEL_H as f32 * scale) as i32;
    let offx = (w as i32 - nw) / 2;
    let offy = (h as i32 - nh) / 2;
    let dtw = ((TERRAIN_TILE_W as f32 * scale).round() as i32).max(1);
    let dth = ((TERRAIN_TILE_H as f32 * scale).round() as i32).max(1);

    let cells = loc.terrain_cells();
    for row in 0..LOCAL_TERRAIN_H {
        for col in 0..LOCAL_TERRAIN_W {
            let idx = (row * LOCAL_TERRAIN_W + col) as usize;
            if idx >= cells.len() {
                continue;
            }
            let (low, high) = terrain_layers(cells[idx]);
            let (lx, ly) = terrain_to_local_px(col, row);
            let dx = offx + (lx as f32 * scale).round() as i32;
            let dy = offy + (ly as f32 * scale).round() as i32;
            for r in [low, high].into_iter().flatten() {
                if let Some(tile) = res.terrain_tile(r) {
                    blit_scaled_nearest(
                        &mut out.data,
                        out.w,
                        out.h,
                        &tile.data,
                        tile.w,
                        tile.h,
                        dx,
                        dy,
                        dtw,
                        dth,
                    );
                }
            }
        }
    }

    for g in &loc.graph_placements {
        let tile = match res.terrain_tile(g.tile as usize) {
            Some(t) => t,
            None => continue,
        };
        let (lx, ly) = graph_anchor_to_local_px(g.x as i32, g.y as i32);
        let dx = offx + (lx as f32 * scale).round() as i32;
        let dy = offy + (ly as f32 * scale).round() as i32;
        blit_scaled_nearest(
            &mut out.data,
            out.w,
            out.h,
            &tile.data,
            tile.w,
            tile.h,
            dx,
            dy,
            dtw,
            dth,
        );
    }

    out
}

/// Full-resolution surface renderer (terrain both layers + GRAPH + props),
/// retained for `examples/dump.rs` and `tools/parity.py`. Native only; this is
/// the static-world oracle and deliberately matches the Python oracle's layer
/// order (terrain low→high, GRAPH, props sorted by projected baseline).
pub fn render_location_surface(res: &mut Res, loc: &Location) -> Image {
    let mut surf = Image::new(
        LOCAL_PIXEL_W as usize,
        LOCAL_PIXEL_H as usize,
        [8, 12, 9, 255],
    );
    let cells = loc.terrain_cells();
    for row in 0..LOCAL_TERRAIN_H {
        for col in 0..LOCAL_TERRAIN_W {
            let idx = (row * LOCAL_TERRAIN_W + col) as usize;
            if idx >= cells.len() {
                continue;
            }
            let (low, high) = terrain_layers(cells[idx]);
            let (x, y) = terrain_to_local_px(col, row);
            for r in [low, high].into_iter().flatten() {
                if let Some(tile) = res.terrain_tile(r) {
                    blit(
                        &mut surf.data,
                        surf.w,
                        surf.h,
                        &tile.data,
                        tile.w,
                        tile.h,
                        x,
                        y,
                    );
                }
            }
        }
    }

    for g in &loc.graph_placements {
        let (x, y) = graph_anchor_to_local_px(g.x as i32, g.y as i32);
        if let Some(tile) = res.terrain_tile(g.tile as usize) {
            blit(
                &mut surf.data,
                surf.w,
                surf.h,
                &tile.data,
                tile.w,
                tile.h,
                x,
                y,
            );
        }
    }

    let mut props: Vec<usize> = (0..loc.props.len()).collect();
    props.sort_by_key(|&i| (loc.props[i].grid_x, loc.props[i].grid_y, loc.props[i].ri));
    for i in props {
        let p = &loc.props[i];
        let (sprite, gx, gy) = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            res.prop_sprite(p.object_id, 0, p.palette)
        })) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let (x, y) = prop_to_local_px(p.grid_x, p.grid_y);
        blit(
            &mut surf.data,
            surf.w,
            surf.h,
            &sprite.data,
            sprite.w,
            sprite.h,
            x + gx,
            y + gy,
        );
    }
    surf
}

/// Bilinear scale into a new RGBA image.
pub fn scale_image(src: &Image, w: usize, h: usize) -> Image {
    let w = w.max(1);
    let h = h.max(1);
    if src.w == w && src.h == h {
        return src.clone();
    }
    let mut out = Image::new(w, h, [0, 0, 0, 0]);
    if src.w == 0 || src.h == 0 {
        return out;
    }
    let sx = src.w as f32 / w as f32;
    let sy = src.h as f32 / h as f32;
    for y in 0..h {
        let fy = (y as f32 + 0.5) * sy - 0.5;
        let y0f = fy.floor();
        let y1f = y0f + 1.0;
        let wy = (fy - y0f).clamp(0.0, 1.0);
        let y0 = y0f.clamp(0.0, src.h as f32 - 1.0) as usize;
        let y1 = y1f.clamp(0.0, src.h as f32 - 1.0) as usize;
        for x in 0..w {
            let fx = (x as f32 + 0.5) * sx - 0.5;
            let x0f = fx.floor();
            let x1f = x0f + 1.0;
            let wx = (fx - x0f).clamp(0.0, 1.0);
            let x0 = x0f.clamp(0.0, src.w as f32 - 1.0) as usize;
            let x1 = x1f.clamp(0.0, src.w as f32 - 1.0) as usize;
            let p00 = (y0 * src.w + x0) * 4;
            let p01 = (y0 * src.w + x1) * 4;
            let p10 = (y1 * src.w + x0) * 4;
            let p11 = (y1 * src.w + x1) * 4;
            let di = (y * w + x) * 4;
            for c in 0..4 {
                let top = src.data[p00 + c] as f32 * (1.0 - wx) + src.data[p01 + c] as f32 * wx;
                let bot = src.data[p10 + c] as f32 * (1.0 - wx) + src.data[p11 + c] as f32 * wx;
                let v = top * (1.0 - wy) + bot * wy;
                out.data[di + c] = v.round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    out
}
