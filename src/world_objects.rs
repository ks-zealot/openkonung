//! Game.N WorldObject table (`0x21900`, 500 x 0x40).
//!
//! The original loader (`FUN_0043A870`) reads the table verbatim into
//! `0x77DEB4`. `FUN_0043ADF0` then:
//!
//! - filters records by the `u16` location at `+0x12`;
//! - applies the activation test on the timestamp pair `+0x00`/`+0x04`;
//! - copies each accepted record into the runtime marker pool `0x785BB4`;
//! - resolves the sprite id (`+0x0C`) through `FUN_00438CC8`.
//!
//! The `+0x14..+0x3F` array is 22 item-instance ids (indexes into the loaded
//! `Game.N` item table, not class/subtype pairs). `+0x10` is signed money.
//!
//! Herb/root nodes (`class 9`, subtype 1/2) use the original respawn path in
//! `FUN_0041093C`: after the sole item is collected, the source record gets a
//! fresh item instance cloned from the collected one and both timestamps are
//! set to `-frame`. `FUN_0043ADF0` accepts it again once
//! `now - deactivated >= 0x1518` frames (5400 frames = 324 s at 60 ms).

use crate::constants::game_layout::{WORLD_OBJECT_COUNT, WORLD_OBJECT_OFF, WORLD_OBJECT_STRIDE};
use crate::res::{i16, u16};

/// Item slots in one WorldObject record.
pub const WORLD_OBJECT_ITEM_SLOTS: usize = 22;
/// Original respawn delay (`0x1518` frames at the 60 ms budget).
pub const WORLD_OBJECT_RESPAWN_FRAMES: i64 = 0x1518;
/// Sprite id used when a record has money or more than one item
/// (`FUN_00438CC8` default branch).
pub const WORLD_OBJECT_GENERIC_SPRITE: u16 = 0xA3;

/// One authored `Game.N +0x21900` record (the canonical source record).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct WorldObjectSource {
    /// Table index, stored at runtime-marker `+0x0E`.
    pub index: usize,
    /// `+0x00` / `+0x04` activation timestamps. Both are `-frame` after the
    /// herb respawn path deactivates the record.
    pub t0: i32,
    pub t4: i32,
    /// `+0x08` / `+0x0A` runtime-grid cell.
    pub x: u16,
    pub y: u16,
    /// `+0x10` signed money.
    pub money: i16,
    /// `+0x12` location id (`0` = permanently removed).
    pub loc: u16,
    /// `+0x14..+0x3F` item-instance ids; `0` is empty.
    pub items: [u16; WORLD_OBJECT_ITEM_SLOTS],
}

impl WorldObjectSource {
    fn from_record(index: usize, r: &[u8]) -> WorldObjectSource {
        let mut items = [0u16; WORLD_OBJECT_ITEM_SLOTS];
        for (i, slot) in items.iter_mut().enumerate() {
            *slot = u16(r, 0x14 + i * 2) as u16;
        }
        WorldObjectSource {
            index,
            t0: i16(r, 0x00) as i32 | ((u16(r, 0x02) as i32) << 16),
            t4: i16(r, 0x04) as i32 | ((u16(r, 0x06) as i32) << 16),
            x: u16(r, 0x08) as u16,
            y: u16(r, 0x0A) as u16,
            money: i16(r, 0x10) as i16,
            loc: u16(r, 0x12) as u16,
            items,
        }
    }

    /// Original `FUN_0043ADF0` activation test for the timestamp pair.
    /// A non-negative `+0x00` or unequal pair is active immediately; an equal
    /// negative pair (`-deactivation_frame`) waits for the `0x1518` cooldown.
    pub fn active_at(&self, now_frames: i64) -> bool {
        if self.loc == 0 {
            return false;
        }
        let t0 = self.t0 as i64;
        let t4 = self.t4 as i64;
        t0 >= 0 || t0 != t4 || now_frames.saturating_add(t0) >= WORLD_OBJECT_RESPAWN_FRAMES
    }

    /// Store `-frame` in both timestamp words (`FUN_0041093C`).
    pub fn deactivate_at(&mut self, now_frames: i64) {
        let stamp = -(now_frames.clamp(0, i32::MAX as i64) as i32);
        self.t0 = stamp;
        self.t4 = stamp;
    }

    /// Permanently remove this source record (`+0x12 = 0`).
    pub fn remove(&mut self) {
        self.loc = 0;
    }

    /// True when this is the single-item herb/root shape that respawns:
    /// exactly one non-zero item, class 9, subtype 1 or 2.
    pub fn is_respawning_herb(&self, item_info: impl Fn(u16) -> Option<(u8, u8)>) -> bool {
        if self.items[0] == 0 || self.items[1..].iter().any(|&v| v != 0) {
            return false;
        }
        matches!(item_info(self.items[0]), Some((9, 1 | 2)))
    }
}

/// One record active on the current map (runtime marker pool `0x785BB4`).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ActiveWorldObject {
    /// Index into the source table.
    pub source: usize,
    pub x: i32,
    pub y: i32,
    pub money: i16,
    /// Non-zero item-instance ids, in original order.
    pub items: Vec<u16>,
    /// Resolved INTR sprite id (`FUN_00438CC8`).
    pub sprite: u16,
}

impl ActiveWorldObject {
    pub fn cell(&self) -> (i32, i32) {
        (self.x, self.y)
    }
}

/// Parse the leading run of `Game.N +0x21900` records, stopping at the first
/// `+0x12 == 0` terminator (max 500), exactly like `FUN_0043A870`.
pub fn parse_sources(save: &[u8]) -> Vec<WorldObjectSource> {
    let mut out = Vec::new();
    let end = save.len();
    for i in 0..WORLD_OBJECT_COUNT {
        let o = WORLD_OBJECT_OFF + i * WORLD_OBJECT_STRIDE;
        if o + WORLD_OBJECT_STRIDE > end {
            break;
        }
        let rec = &save[o..o + WORLD_OBJECT_STRIDE];
        if u16(rec, 0x12) == 0 {
            break;
        }
        out.push(WorldObjectSource::from_record(i, rec));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_stops_at_location_terminator_and_keeps_22_items() {
        let mut data = vec![0u8; WORLD_OBJECT_OFF + WORLD_OBJECT_STRIDE * 3];
        let r0 = WORLD_OBJECT_OFF;
        data[r0 + 0x12] = 3;
        data[r0 + 0x08] = 10;
        data[r0 + 0x0A] = 20;
        data[r0 + 0x14] = 7;
        data[r0 + 0x15] = 0;
        let r1 = r0 + WORLD_OBJECT_STRIDE;
        data[r1 + 0x12] = 4;
        // Third record is the terminator.
        let recs = parse_sources(&data);
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[0].loc, 3);
        assert_eq!(recs[0].items[0], 7);
        assert_eq!(recs[1].loc, 4);
    }

    #[test]
    fn real_game1_herb_placements_match_the_original_table() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let Ok(game) = std::fs::read(dir.join("Game.1")) else {
            eprintln!("skipping: Game.1 not present");
            return;
        };
        let Ok(konung) = std::fs::read(dir.join("konung.res")) else {
            return;
        };
        let db = crate::item::ItemDb::from_konung(&konung);
        let sources = parse_sources(&game);
        let mut herbs = 0usize;
        let mut loc3 = 0usize;
        let mut loc4 = 0usize;
        for s in &sources {
            if !s.is_respawning_herb(|id| {
                crate::item::ItemDb::decode_instance(&game, id).map(|i| (i.class, i.subtype))
            }) {
                continue;
            }
            herbs += 1;
            match s.loc {
                3 => loc3 += 1,
                4 => loc4 += 1,
                _ => {}
            }
        }
        assert_eq!(herbs, 95);
        assert_eq!(loc3, 19);
        assert_eq!(loc4, 5);
        // The two names/species are class 9 subtypes 1 and 2.
        assert_eq!(db.info(9, 1).unwrap().name, "Земляной орех");
        assert_eq!(db.info(9, 2).unwrap().name, "Белый корень");
    }

    #[test]
    fn activation_and_respawn_timestamps() {
        let mut s = WorldObjectSource {
            index: 0,
            t0: 0,
            t4: 0,
            x: 1,
            y: 2,
            money: 0,
            loc: 5,
            items: [0; WORLD_OBJECT_ITEM_SLOTS],
        };
        assert!(s.active_at(0));
        s.deactivate_at(100);
        assert!(!s.active_at(100 + WORLD_OBJECT_RESPAWN_FRAMES - 1));
        assert!(s.active_at(100 + WORLD_OBJECT_RESPAWN_FRAMES));
        s.remove();
        assert!(!s.active_at(1_000_000));
    }
}
