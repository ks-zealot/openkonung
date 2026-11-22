//! World lighting (`FUN_00428FE0` ambient + `FUN_0042532C` LIGHTS maps).
//!
//! The original renders every element through a **palette version** chosen at
//! draw time:
//!
//! * terrain and GRAPH placements: ambient delta (`FUN_0043D971`) and, on
//!   `LIGHTS.N` low-map cells, the `LIGHTS.RES` 114x64 re-light masks;
//! * selection rings: base palette (never ambient-shaded);
//! * ground markers: ambient unless a torch is out (then the base palette over
//!   the light mask);
//! * props/actors: ambient, except sprites on a cell whose `LIGHTS.N` high-map
//!   byte is non-zero, which use the **unshaded** palette; projectiles use the
//!   base palette (`FUN_0043CEAA(..., DAT_005076f4, ...)`).
//!
//! The remake has decoded RGBA pixels, so the background pass shades the
//! finished terrain once, and [`PlayableApp::sprite_ambient`] +
//! `Res::blit_shaded` reproduce the per-sprite palette choice at blit time.

use super::*;
use crate::lighting::{ambient, shade_rgb};
use crate::loc::{terrain_to_local_px, LOCAL_TERRAIN_H, LOCAL_TERRAIN_W};

impl PlayableApp {
    /// Override the world frame counter that drives the `FUN_00428FE0` ambient
    /// palette (diagnostics and the lighting smoke).
    pub fn set_world_frame(&mut self, frame: u64) {
        self.world_frame = frame;
    }

    /// The world frame counter (`_DAT_0081D640` equivalent) last used by the
    /// ambient pass.
    pub fn world_frame(&self) -> u64 {
        self.world_frame
    }

    /// Spans emitted by the last shadow pass (`FUN_0043C040`).
    pub fn shadow_span_count(&self) -> u32 {
        self.shadow_spans as u32
    }

    /// One field (0 row, 1 x0, 2 x1) of a span kept by the diagnostic ABI.
    pub fn shadow_span(&self, index: usize, field: u32) -> Option<i32> {
        let s = self.shadow_debug.get(index)?;
        Some(match field {
            0 => s.row,
            1 => s.x0,
            2 => s.x1,
            _ => -1,
        })
    }

    /// Apply ambient darkening, terrain light masks and the lit-sprite palette
    /// choice.
    ///
    /// Three decisions of the original palette versions are reproduced:
    ///
    /// 1. every framebuffer pixel is shaded by the `FUN_00428FE0` ambient delta
    ///    (zero while `day_night` is off; **not** gated by `light_effects`,
    ///    because `FUN_0042863C` stores the delta in `0x5076BC` unconditionally);
    /// 2. a `LIGHTS.N` low-map cell with a non-zero id re-lights its terrain tile
    ///    through the `LIGHTS.RES` 114x64 mask, only while `light_effects` is on
    ///    and the phase is in the night window (`_DAT_0082D168`).
    ///
    /// The per-element palette choice (3) is applied at blit time by
    /// [`PlayableApp::sprite_ambient`] + `blit_shaded`, so the background pass
    /// must run *before* the unshaded overlays (selection rings) and the
    /// ambient-shaded ground markers, and before the sprites.
    ///
    /// `ox`/`oy` are the camera pan of the finished background so both passes
    /// land on the same pixels.
    pub(super) fn apply_background_lighting(&mut self, ox: i32, oy: i32) {
        let cfg = crate::config::config();
        let amb = ambient(self.world_frame, cfg.day_night);
        if amb.is_zero() {
            return;
        }
        // The ambient palette delta is applied to terrain, props and entities
        // regardless of the `light_effects` option: `FUN_0042863C` stores the
        // `FUN_00428FE0` result in `0x5076BC` unconditionally, and
        // `FUN_0043D971` picks the shaded palette version from it. The option
        // only gates the `LIGHTS.N` mask/re-light pass.
        //
        // `FUN_0042532C` enters that pass only when the night flag
        // `0x82D168 != 0` **and** `light_effects` (`0x81D604`) is on
        // (`0x425354..0x425364`). The low-map byte selects a 114x64
        // `LIGHTS.RES` tile whose bytes are the light intensity `L` (0..31);
        // `FUN_0043CC49` then rebuilds red/green through the ambient ramp whose
        // index is `|delta| - L` (so the cell is *less* dark) after the
        // per-frame fire flicker lift, while blue keeps the plain ambient ramp.
        // See `lighting::shade_lit`.
        self.light_frame.fill(0);
        let mut flicker = crate::lighting::Flicker::NONE;
        if cfg.light_effects != 0 && crate::lighting::is_night_phase(self.world_frame) {
            self.build_light_frame(ox, oy);
            flicker = crate::lighting::Flicker::roll(&mut self.combat_rng).halved();
        }
        {
            let light = &self.light_frame;
            for (i, px) in self.frame.chunks_exact_mut(4).enumerate() {
                let l = light[i];
                let [r, g, b] = if l == 0 {
                    shade_rgb([px[0], px[1], px[2]], amb)
                } else {
                    crate::lighting::shade_lit([px[0], px[1], px[2]], amb, l, flicker)
                };
                px[0] = r;
                px[1] = g;
                px[2] = b;
            }
        }
    }

    /// Build the per-pixel `LIGHTS.N` light intensity for this frame.
    ///
    /// Mirrors the `FUN_0042532C` night loop: for every low-map cell with a
    /// non-zero id, blit the 114x64 `LIGHTS.RES` tile at the terrain tile's
    /// screen origin; a non-zero byte of a later cell overwrites an earlier one
    /// (a zero byte is transparent). The tile bytes are already the 0..31
    /// `L` that [`crate::lighting::shade_lit`] subtracts from the ambient ramp
    /// index, so they are stored verbatim.
    fn build_light_frame(&mut self, ox: i32, oy: i32) {
        let fw = VIRTUAL_W as usize;
        let PlayableApp {
            world,
            light_frame,
            current_map,
            ..
        } = self;
        let Some(map) = world.light_map(*current_map) else {
            return;
        };
        for row in 0..LOCAL_TERRAIN_H {
            for col in 0..LOCAL_TERRAIN_W {
                let value = map.low_at(col, row);
                if value == 0 {
                    continue;
                }
                let Some(tile) = world.res.light_tile(value) else {
                    continue;
                };
                let (tw, th) = (tile.w, tile.h);
                let (lx, ly) = terrain_to_local_px(col, row);
                let sx = ox + lx;
                let sy = oy + ly;
                if sx >= VIRTUAL_W as i32
                    || sy >= VIRTUAL_H as i32
                    || sx + tw as i32 <= 0
                    || sy + th as i32 <= 0
                {
                    continue;
                }
                for y in 0..th {
                    let py = sy + y as i32;
                    if py < 0 || py >= VIRTUAL_H as i32 {
                        continue;
                    }
                    for x in 0..tw {
                        let px = sx + x as i32;
                        if px < 0 || px >= VIRTUAL_W as i32 {
                            continue;
                        }
                        let v = tile.data[y * tw + x];
                        if v == 0 {
                            continue;
                        }
                        light_frame[py as usize * fw + px as usize] = v;
                    }
                }
            }
        }
    }

    /// Ambient palette delta currently in effect (zero on a neutral phase or
    /// with the day/night cycle disabled).
    ///
    /// `light_effects` does **not** clear it: `FUN_0043D971` applies the ambient
    /// delta to every palette regardless of that option; only the `LIGHTS.N`
    /// re-light masks are gated by it.
    pub(super) fn ambient_delta(&self) -> crate::lighting::Ambient {
        let cfg = crate::config::config();
        ambient(self.world_frame, cfg.day_night)
    }

    /// Ambient palette delta for one sprite at its local projection.
    ///
    /// The original picks a palette version per draw: `FUN_0043D971` (ambient)
    /// unless the sprite's cell has a non-zero `LIGHTS.N` high-map byte, which
    /// selects the **unshaded** palette (`DAT_005076f4 + palette`). Returns the
    /// neutral delta for the unshaded case so callers can use the plain blit.
    pub(super) fn sprite_ambient(&self, lx: f64, ly: f64) -> crate::lighting::Ambient {
        let delta = self.ambient_delta();
        if delta.is_zero() {
            return delta;
        }
        let Some(map) = self.world.light_map(self.current_map) else {
            return delta;
        };
        let (cx, cy) =
            crate::coords::local_px_to_entity(lx.round() as i32, ly.round() as i32);
        if map.high_at(cx, cy) != 0 {
            return crate::lighting::Ambient::default();
        }
        delta
    }
}
