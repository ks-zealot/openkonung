//! `LIGHTS.RES` / `LIGHTS.N` visual-light data.
//!
//! `FUN_004392B4` reads `LIGHTS.RES` as:
//!
//! ```text
//! 0x1000  -> 64 small 8x8 light sprites
//! 0x0AC0  -> one 64x43 light mask
//! 19 x 0x1C80 -> 19 light tiles (114x64)
//! ```
//!
//! The shipped file is 145472 bytes; the original loader reads 145088 and
//! leaves the final 384 bytes unread.
//!
//! Each `LIGHTS.N` map file is exactly 64000 bytes: an 80x160 low-resolution
//! light-index map (`0x3200`) followed by a 320x160 per-cell map (`0xC800`).

use super::Res;

pub const LIGHTS_RES_READ: usize = 0x1000 + 0x0AC0 + 19 * 0x1C80;
pub const LIGHT_TILE_COUNT: usize = 19;
pub const LIGHT_TILE_W: usize = 114;
pub const LIGHT_TILE_H: usize = 64;
pub const LIGHT_SMALL_COUNT: usize = 64;
pub const LIGHT_SMALL_W: usize = 8;
pub const LIGHT_SMALL_H: usize = 8;
pub const LIGHT_LARGE_W: usize = 64;
pub const LIGHT_LARGE_H: usize = 43;
pub const LIGHT_MAP_LEN: usize = 0x3200 + 0xC800;
pub const LIGHT_LOW_W: usize = 80;
pub const LIGHT_LOW_H: usize = 160;
pub const LIGHT_HIGH_W: usize = 320;
pub const LIGHT_HIGH_H: usize = 160;

/// One 8-bit light/alpha mask. The original blits these through its light
/// palette; the remake uses the byte as an intensity multiplier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LightMask {
    pub w: usize,
    pub h: usize,
    pub data: Vec<u8>,
}

impl LightMask {
    pub fn get(&self, x: usize, y: usize) -> u8 {
        if x >= self.w || y >= self.h {
            return 0;
        }
        self.data[y * self.w + x]
    }
}

/// Decoded `LIGHTS.RES`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LightRes {
    pub small: Vec<LightMask>,
    pub large: LightMask,
    pub tiles: Vec<LightMask>,
}

impl LightRes {
    pub fn parse(data: &[u8]) -> Result<LightRes, String> {
        if data.len() < LIGHTS_RES_READ {
            return Err(format!(
                "LIGHTS.RES: file is {} bytes, need at least {}",
                data.len(),
                LIGHTS_RES_READ
            ));
        }
        let mut small = Vec::with_capacity(LIGHT_SMALL_COUNT);
        for i in 0..LIGHT_SMALL_COUNT {
            let o = i * LIGHT_SMALL_W * LIGHT_SMALL_H;
            small.push(LightMask {
                w: LIGHT_SMALL_W,
                h: LIGHT_SMALL_H,
                data: data[o..o + LIGHT_SMALL_W * LIGHT_SMALL_H].to_vec(),
            });
        }
        let large_start = LIGHT_SMALL_COUNT * LIGHT_SMALL_W * LIGHT_SMALL_H;
        let large_end = large_start + LIGHT_LARGE_W * LIGHT_LARGE_H;
        let large = LightMask {
            w: LIGHT_LARGE_W,
            h: LIGHT_LARGE_H,
            data: data[large_start..large_end].to_vec(),
        };
        let mut tiles = Vec::with_capacity(LIGHT_TILE_COUNT);
        let base = large_end;
        for i in 0..LIGHT_TILE_COUNT {
            let o = base + i * LIGHT_TILE_W * LIGHT_TILE_H;
            tiles.push(LightMask {
                w: LIGHT_TILE_W,
                h: LIGHT_TILE_H,
                data: data[o..o + LIGHT_TILE_W * LIGHT_TILE_H].to_vec(),
            });
        }
        Ok(LightRes {
            small,
            large,
            tiles,
        })
    }

    pub fn tile(&self, index: u8) -> Option<&LightMask> {
        if index == 0 {
            return None;
        }
        self.tiles.get(index as usize - 1)
    }
}

impl Res {
    /// Decode and attach `LIGHTS.RES`. Returns the number of 114x64 tiles.
    pub fn load_lights(&mut self, data: &[u8]) -> Result<usize, String> {
        let lights = LightRes::parse(data)?;
        let count = lights.tiles.len();
        self.lights = Some(lights);
        Ok(count)
    }

    pub fn light_tile(&self, index: u8) -> Option<&LightMask> {
        self.lights.as_ref().and_then(|l| l.tile(index))
    }
}

/// Per-map light index maps from `LIGHTS.N`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LightMap {
    pub low: Vec<u8>,
    pub high: Vec<u8>,
}

impl LightMap {
    pub fn parse(data: &[u8]) -> Result<LightMap, String> {
        if data.len() < LIGHT_MAP_LEN {
            return Err(format!(
                "LIGHTS.N: file is {} bytes, need exactly {}",
                data.len(),
                LIGHT_MAP_LEN
            ));
        }
        Ok(LightMap {
            low: data[..0x3200].to_vec(),
            high: data[0x3200..LIGHT_MAP_LEN].to_vec(),
        })
    }

    pub fn zero() -> LightMap {
        LightMap {
            low: vec![0; 0x3200],
            high: vec![0; 0xC800],
        }
    }

    /// Low map is `160 rows x 80 columns`, indexed `row*80 + col` (the
    /// original `FUN_0042532C` reads `&0x650318 + col + row*0x50`).
    pub fn low_at(&self, col: i32, row: i32) -> u8 {
        if col < 0 || row < 0 || col >= LIGHT_LOW_W as i32 || row >= LIGHT_LOW_H as i32 {
            return 0;
        }
        self.low[row as usize * LIGHT_LOW_W + col as usize]
    }

    pub fn high_at(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x >= LIGHT_HIGH_W as i32 || y >= LIGHT_HIGH_H as i32 {
            return 0;
        }
        self.high[x as usize * LIGHT_HIGH_H + y as usize]
    }
}

/// Unit helper used by tests/tools: convert a light index to a tint scale.
pub fn light_scale(value: u8) -> f32 {
    if value == 0 {
        1.0
    } else {
        0.35 + (value.min(18) as f32 / 18.0) * 0.65
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_lights_res_has_expected_masks() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("LIGHTS.RES");
        let Ok(bytes) = std::fs::read(path) else {
            return;
        };
        let lights = LightRes::parse(&bytes).expect("LIGHTS.RES");
        assert_eq!(lights.small.len(), LIGHT_SMALL_COUNT);
        assert_eq!(
            (lights.large.w, lights.large.h),
            (LIGHT_LARGE_W, LIGHT_LARGE_H)
        );
        assert_eq!(lights.tiles.len(), LIGHT_TILE_COUNT);
        assert_eq!(
            (lights.tiles[0].w, lights.tiles[0].h),
            (LIGHT_TILE_W, LIGHT_TILE_H)
        );
    }

    #[test]
    fn light_map_split_is_exact() {
        let mut data = vec![0u8; LIGHT_MAP_LEN];
        data[0] = 3;
        data[0x3200] = 7;
        let map = LightMap::parse(&data).expect("map");
        assert_eq!(map.low_at(0, 0), 3);
        assert_eq!(map.high_at(0, 0), 7);
        assert_eq!(map.high.len(), 0xC800);
    }
}
