//! World lighting pass (`FUN_00428FE0` ambient + `FUN_0042532C` LIGHTS maps).
//!
//! The original renders terrain and entity sprites through palette versions:
//! at night the ambient delta is baked into the palette, `LIGHTS.N` low-map
//! cells re-light terrain with the `LIGHTS.RES` 114x64 masks, and entity cells
//! whose high-map byte is non-zero use the unshaded palette.
//!
//! The remake already has decoded RGBA sprites, so this pass applies the same
//! three decisions directly to the framebuffer: shade the finished world, blend
//! the light masks back over lit terrain cells, then redraw sprites standing on
//! lit high-map cells with their original colors.

use super::*;
use crate::lighting::{ambient, light_pixel, shade_channel};
use crate::loc::{terrain_to_local_px, LOCAL_TERRAIN_H, LOCAL_TERRAIN_W};
use crate::render::WorldSprite;
use crate::res::{TERRAIN_TILE_H, TERRAIN_TILE_W};

impl PlayableApp {
    pub fn set_world_frame(&mut self, frame: u64) {
        self.world_frame = frame;
    }

    pub fn world_frame(&self) -> u64 {
        self.world_frame
    }

    pub(super) fn apply_world_lighting(
        &mut self,
        sprites: &[WorldSprite],
        ox: i32,
        oy: i32,
    ) {
        let cfg = crate::config::config();
        if cfg.light_effects == 0 {
            return;
        }
        let amb = ambient(self.world_frame, cfg.day_night);
        if amb.is_zero() {
            return;
        }
        let fw = VIRTUAL_W as usize;
        let fh = VIRTUAL_H as usize;
        for px in self.frame.chunks_exact_mut(4) {
            px[0] = shade_channel(px[0], amb.r);
            px[1] = shade_channel(px[1], amb.g);
            px[2] = shade_channel(px[2], amb.b);
        }

        let Some(map) = self.world.light_map(self.current_map) else {
            return;
        };
        // Terrain light masks (`LIGHTS.N` low map, 160 rows x 80 columns).
        for row in 0..LOCAL_TERRAIN_H {
            for col in 0..LOCAL_TERRAIN_W {
                let value = map.low_at(col, row);
                if value == 0 {
                    continue;
                }
                let (lx, ly) = terrain_to_local_px(col, row);
                let sx = ox + lx;
                let sy = oy + ly;
                if sx >= VIRTUAL_W || sy >= VIRTUAL_H || sx + TERRAIN_TILE_W <= 0 || sy + TERRAIN_TILE_H <= 0 {
                    continue;
                }
                // Clone the mask row data so the frame/world borrows stay split.
                let tile = self
                    .world
                    .res
                    .light_tile(value)
                    .map(|t| (t.w, t.h, t.data.clone()));
                let Some((tw, th, data)) = tile else {
                    continue;
                };
                for y in 0..th {
                    let py = sy + y as i32;
                    if py < 0 || py >= VIRTUAL_H {
                        continue;
                    }
                    for x in 0..tw {
                        let px = sx + x as i32;
                        if px < 0 || px >= VIRTUAL_W {
                            continue;
                        }
                        let a = data[y * tw + x];
                        if a == 0 {
                            continue;
                        }
                        let o = (py as usize * fw + px as usize) * 4;
                        let [r, g, b, al] = [self.frame[o], self.frame[o + 1], self.frame[o + 2], self.frame[o + 3]];
                        let lit = light_pixel([r, g, b, al], a);
                        self.frame[o..o + 4].copy_from_slice(&lit);
                    }
                }
            }
        }

        // Lit entity cells use the unshaded palette in the original.
        for s in sprites {
            let (cx, cy) = crate::coords::local_px_to_entity(s.lx.round() as i32, s.ly.round() as i32);
            if map.high_at(cx, cy) == 0 {
                continue;
            }
            let ax = ox + s.lx.round() as i32 + s.gx;
            let ay = oy + s.ly.round() as i32 + s.gy;
            blit(&mut self.frame, fw, fh, &s.img.data, s.img.w, s.img.h, ax, ay);
        }
    }
}
