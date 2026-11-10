//! Legacy `KONUNG.SAV` / `KONUNG.SA0..9` import and export.
//!
//! The original writer `FUN_00424B3C` and reader `FUN_004246C0` use one flat
//! little-endian block sequence with no header, version or checksum
//! (`KONUNG_SAVE_LOAD.md` section 4.1). The remake keeps its richer native
//! `OPKS` payload as the canonical browser format; this module imports a real
//! snapshot and writes one back so saves can move between the original game and
//! the remake.
//!
//! Blocks whose runtime layout equals the `Game.N` layout (groups, entities,
//! world objects, settlements) are exported by starting from the loaded
//! `Game.N` bytes and patching every field the remake models, which preserves
//! the bytes the remake does not interpret. The projectile pool has no
//! recovered field map beyond its 100 x `0x20` shape, so exports carry an empty
//! pool.

use crate::constants::game_layout::{
    ENTITY_COUNT, ENTITY_STRIDE, GROUP_COUNT, GROUP_STRIDE, SETTLEMENT_COUNT, SETTLEMENT_STRIDE,
    WAYPOINT_MAX, WAYPOINT_OFF, WAYPOINT_STRIDE, WORLD_OBJECT_COUNT, WORLD_OBJECT_STRIDE,
};
use crate::population::{
    parse_entity_record, parse_group_record, parse_settlement_record, Entity, Group, Settlement,
    SettlementTrade,
};
use crate::world_objects::{ActiveWorldObject, WorldObjectSource, WORLD_OBJECT_ITEM_SLOTS};

/// Exact `KONUNG.SA0..9` payload size (`0x0B07A4`).
pub const SNAPSHOT_LEN: usize = 0x0B07A4;
/// `KONUNG.SAV`: ten slots A- 50 bytes.
pub const SLOT_NAMES_LEN: usize = 500;
pub const SLOT_NAME_LEN: usize = 50;
pub const SLOT_COUNT: usize = 10;

pub const OPT2_OFF: usize = 0x000000;
pub const OPT3_OFF: usize = 0x000004;
pub const WORLD_TICK_OFF: usize = 0x000008;
pub const LEADER_OFF: usize = 0x00000C;
pub const PROJECTILE_COUNT_OFF: usize = 0x000010;
pub const MARKER_COUNT_OFF: usize = 0x000014;
pub const QUEST_OFF: usize = 0x000018;
pub const QUEST_LEN: usize = 0x4B0;
pub const PROJECTILE_OFF: usize = 0x0004C8;
pub const PROJECTILE_LEN: usize = 0xC80;
pub const ITEM_TABLE_OFF: usize = 0x001148;
pub const ITEM_TABLE_LEN: usize = 0x20000;
pub const MARKER_OFF: usize = 0x021148;
pub const MARKER_LEN: usize = 0x3200;
pub const GROUP_BLOCK_OFF: usize = 0x024348;
pub const GROUP_BLOCK_LEN: usize = GROUP_COUNT * GROUP_STRIDE;
pub const WORLD_OBJECT_BLOCK_OFF: usize = 0x025C48;
pub const WORLD_OBJECT_BLOCK_LEN: usize = WORLD_OBJECT_COUNT * WORLD_OBJECT_STRIDE;
pub const ENTITY_BLOCK_OFF: usize = 0x02D948;
pub const ENTITY_BLOCK_LEN: usize = ENTITY_COUNT * ENTITY_STRIDE;
pub const SETTLEMENT_BLOCK_OFF: usize = 0x0AA948;
pub const SETTLEMENT_BLOCK_LEN: usize = SETTLEMENT_COUNT * SETTLEMENT_STRIDE;
pub const DISCOVERED_OFF: usize = 0x0B076C;
pub const DISCOVERED_LEN: usize = 0x38;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Block {
    pub off: usize,
    pub len: usize,
}

pub const QUEST_BLOCK: Block = Block {
    off: QUEST_OFF,
    len: QUEST_LEN,
};
pub const PROJECTILE_BLOCK: Block = Block {
    off: PROJECTILE_OFF,
    len: PROJECTILE_LEN,
};
pub const ITEM_TABLE_BLOCK: Block = Block {
    off: ITEM_TABLE_OFF,
    len: ITEM_TABLE_LEN,
};
pub const MARKER_BLOCK: Block = Block {
    off: MARKER_OFF,
    len: MARKER_LEN,
};
pub const GROUP_BLOCK: Block = Block {
    off: GROUP_BLOCK_OFF,
    len: GROUP_BLOCK_LEN,
};
pub const WORLD_OBJECT_BLOCK: Block = Block {
    off: WORLD_OBJECT_BLOCK_OFF,
    len: WORLD_OBJECT_BLOCK_LEN,
};
pub const ENTITY_BLOCK: Block = Block {
    off: ENTITY_BLOCK_OFF,
    len: ENTITY_BLOCK_LEN,
};
pub const SETTLEMENT_BLOCK: Block = Block {
    off: SETTLEMENT_BLOCK_OFF,
    len: SETTLEMENT_BLOCK_LEN,
};
pub const DISCOVERED_BLOCK: Block = Block {
    off: DISCOVERED_OFF,
    len: DISCOVERED_LEN,
};

/// One parsed `KONUNG.SAn` snapshot. Blocks are kept verbatim so unmodelled
/// bytes survive a parse/write round-trip.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegacySnapshot {
    pub opt2: u32,
    pub opt3: u32,
    pub world_tick: u32,
    pub leader_index: u32,
    pub projectile_count: u32,
    pub marker_count: u32,
    pub quest_flags: Vec<u32>,
    pub projectiles: Vec<u8>,
    pub item_table: Vec<u8>,
    pub markers: Vec<u8>,
    pub groups: Vec<u8>,
    pub world_objects: Vec<u8>,
    pub entities: Vec<u8>,
    pub settlements: Vec<u8>,
    pub discovered: Vec<u8>,
}

fn u32_at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

fn put_u16(b: &mut [u8], o: usize, v: u16) {
    b[o..o + 2].copy_from_slice(&v.to_le_bytes());
}

fn put_i16(b: &mut [u8], o: usize, v: i16) {
    b[o..o + 2].copy_from_slice(&v.to_le_bytes());
}

fn put_u32(b: &mut [u8], o: usize, v: u32) {
    b[o..o + 4].copy_from_slice(&v.to_le_bytes());
}

fn put_i32(b: &mut [u8], o: usize, v: i32) {
    b[o..o + 4].copy_from_slice(&v.to_le_bytes());
}

impl LegacySnapshot {
    /// A structurally valid all-zero snapshot (used as the export base).
    pub fn empty() -> LegacySnapshot {
        let zeros = |n: usize| vec![0u8; n];
        LegacySnapshot {
            opt2: 1,
            opt3: 1,
            world_tick: 0,
            leader_index: 0,
            projectile_count: 0,
            marker_count: 0,
            quest_flags: vec![0; 300],
            projectiles: zeros(PROJECTILE_LEN),
            item_table: zeros(ITEM_TABLE_LEN),
            markers: zeros(MARKER_LEN),
            groups: zeros(GROUP_BLOCK_LEN),
            world_objects: zeros(WORLD_OBJECT_BLOCK_LEN),
            entities: zeros(ENTITY_BLOCK_LEN),
            settlements: zeros(SETTLEMENT_BLOCK_LEN),
            discovered: zeros(DISCOVERED_LEN),
        }
    }

    /// Parse the exact flat layout. A short file is rejected (the original would
    /// leave globals half-overwritten; the remake fails loudly instead).
    pub fn parse(bytes: &[u8]) -> Result<LegacySnapshot, String> {
        if bytes.len() < SNAPSHOT_LEN {
            return Err(format!(
                "legacy snapshot is {} bytes, expected {} (0x{:X})",
                bytes.len(),
                SNAPSHOT_LEN,
                SNAPSHOT_LEN
            ));
        }
        let block = |b: Block| bytes[b.off..b.off + b.len].to_vec();
        let mut quest_flags = Vec::with_capacity(300);
        for i in 0..300 {
            quest_flags.push(u32_at(bytes, QUEST_OFF + i * 4));
        }
        Ok(LegacySnapshot {
            opt2: u32_at(bytes, OPT2_OFF),
            opt3: u32_at(bytes, OPT3_OFF),
            world_tick: u32_at(bytes, WORLD_TICK_OFF),
            leader_index: u32_at(bytes, LEADER_OFF),
            projectile_count: u32_at(bytes, PROJECTILE_COUNT_OFF),
            marker_count: u32_at(bytes, MARKER_COUNT_OFF),
            quest_flags,
            projectiles: block(PROJECTILE_BLOCK),
            item_table: block(ITEM_TABLE_BLOCK),
            markers: block(MARKER_BLOCK),
            groups: block(GROUP_BLOCK),
            world_objects: block(WORLD_OBJECT_BLOCK),
            entities: block(ENTITY_BLOCK),
            settlements: block(SETTLEMENT_BLOCK),
            discovered: block(DISCOVERED_BLOCK),
        })
    }

    pub fn write(&self) -> Vec<u8> {
        let mut out = vec![0u8; SNAPSHOT_LEN];
        put_u32(&mut out, OPT2_OFF, self.opt2);
        put_u32(&mut out, OPT3_OFF, self.opt3);
        put_u32(&mut out, WORLD_TICK_OFF, self.world_tick);
        put_u32(&mut out, LEADER_OFF, self.leader_index);
        put_u32(
            &mut out,
            PROJECTILE_COUNT_OFF,
            self.projectile_count.min(100),
        );
        put_u32(&mut out, MARKER_COUNT_OFF, self.marker_count.min(200));
        for (i, v) in self.quest_flags.iter().take(300).enumerate() {
            put_u32(&mut out, QUEST_OFF + i * 4, *v);
        }
        for (b, dst) in [
            (PROJECTILE_BLOCK, &self.projectiles),
            (ITEM_TABLE_BLOCK, &self.item_table),
            (MARKER_BLOCK, &self.markers),
            (GROUP_BLOCK, &self.groups),
            (WORLD_OBJECT_BLOCK, &self.world_objects),
            (ENTITY_BLOCK, &self.entities),
            (SETTLEMENT_BLOCK, &self.settlements),
            (DISCOVERED_BLOCK, &self.discovered),
        ] {
            let n = dst.len().min(b.len);
            out[b.off..b.off + n].copy_from_slice(&dst[..n]);
        }
        out
    }

    /// Parse the group block back into `Group[200]` (runtime layout).
    pub fn groups(&self) -> Vec<Group> {
        (0..GROUP_COUNT)
            .map(|i| {
                let o = i * GROUP_STRIDE;
                parse_group_record(i, &self.groups[o..o + GROUP_STRIDE])
            })
            .collect()
    }

    /// Parse the entity block back into `Entity[2000]` (runtime layout).
    pub fn entities(&self) -> Vec<Entity> {
        (0..ENTITY_COUNT)
            .map(|i| {
                let o = i * ENTITY_STRIDE;
                parse_entity_record(i, &self.entities[o..o + ENTITY_STRIDE])
            })
            .collect()
    }

    pub fn settlements(&self) -> Vec<Settlement> {
        (0..SETTLEMENT_COUNT)
            .map(|i| {
                let o = i * SETTLEMENT_STRIDE;
                parse_settlement_record(i, &self.settlements[o..o + SETTLEMENT_STRIDE])
            })
            .collect()
    }

    /// Settlement trade/restock fields from the same block.
    pub fn trades(&self) -> Vec<SettlementTrade> {
        (0..SETTLEMENT_COUNT)
            .map(|i| {
                let o = i * SETTLEMENT_STRIDE;
                crate::population::parse_settlement_trade_record(
                    i,
                    &self.settlements[o..o + SETTLEMENT_STRIDE],
                )
            })
            .collect()
    }

    /// Runtime marker pool (`0x785BB4`): `+0x0E` is the source index.
    pub fn markers(&self) -> Vec<ActiveWorldObject> {
        let count = (self.marker_count as usize).min(200);
        let mut out = Vec::with_capacity(count);
        for i in 0..count {
            let o = i * 0x40;
            let r = &self.markers[o..o + 0x40];
            let mut items = Vec::new();
            for slot in 0..WORLD_OBJECT_ITEM_SLOTS {
                let id = u16::from_le_bytes([r[0x14 + slot * 2], r[0x14 + slot * 2 + 1]]);
                if id != 0 {
                    items.push(id);
                }
            }
            out.push(ActiveWorldObject {
                source: u16::from_le_bytes([r[0x0E], r[0x0F]]) as usize,
                x: u16::from_le_bytes([r[0x08], r[0x09]]) as i32,
                y: u16::from_le_bytes([r[0x0A], r[0x0B]]) as i32,
                money: i16::from_le_bytes([r[0x10], r[0x11]]),
                items,
                sprite: u16::from_le_bytes([r[0x0C], r[0x0D]]),
            });
        }
        out
    }

    pub fn leader(&self) -> Option<Entity> {
        let idx = self.leader_index as usize;
        if idx >= ENTITY_COUNT {
            return None;
        }
        let o = idx * ENTITY_STRIDE;
        Some(parse_entity_record(
            idx,
            &self.entities[o..o + ENTITY_STRIDE],
        ))
    }
}

/// Patch a `Game.N`-shaped group block from live groups (preserves unmodelled
/// bytes).
pub fn patch_groups(mut block: Vec<u8>, groups: &[Group]) -> Vec<u8> {
    for (i, g) in groups.iter().take(GROUP_COUNT).enumerate() {
        let o = i * GROUP_STRIDE;
        let r = &mut block[o..o + GROUP_STRIDE];
        put_u16(r, 0x00, g.first_entity);
        put_i32(r, 0x02, g.growth_timer);
        put_i16(r, 0x06, g.growth_countdown);
        put_u16(r, 0x08, g.rect_a.x1);
        put_u16(r, 0x0A, g.rect_b.x1);
        put_u16(r, 0x0C, g.rect_a.x2);
        put_u16(r, 0x0E, g.rect_b.x2);
        put_u16(r, 0x10, g.rect_a.y1);
        put_u16(r, 0x12, g.rect_b.y1);
        put_u16(r, 0x14, g.rect_a.y2);
        put_u16(r, 0x16, g.rect_b.y2);
        r[0x18] = g.count_18;
        r[0x19] = g.capacity;
        r[0x1A] = g.target_group;
        r[0x1B] = g.loc_id;
        r[0x1C] = g.entity_count;
        r[0x1D] = g.activity_1d;
        r[0x1E] = g.class_1e;
        r[0x1F] = u8::from(g.visited);
    }
    block
}

/// Patch a `Game.N`-shaped entity block from live entities.
pub fn patch_entities(mut block: Vec<u8>, entities: &[Entity]) -> Vec<u8> {
    for (i, e) in entities.iter().take(ENTITY_COUNT).enumerate() {
        let o = i * ENTITY_STRIDE;
        let r = &mut block[o..o + ENTITY_STRIDE];
        let s = e.stats;
        r[0x00] = e.kind;
        r[0x01] = e.facing;
        r[0x02] = e.flags_a;
        r[0x03] = e.flags_b;
        r[0x04] = e.archetype;
        r[0x05] = e.group_id;
        put_u16(r, 0x08, e.x);
        put_u16(r, 0x0A, e.y);
        r[0x11] = s.raw_0x11;
        r[0x12] = e.raw_0x12;
        r[0x33] = s.level;
        put_i32(r, 0x34, s.money);
        put_u32(r, 0x38, s.available_xp);
        put_u32(r, 0x3C, s.invested_xp);
        put_u16(r, 0x40, s.melee_mastery);
        put_u16(r, 0x42, s.trade);
        put_u16(r, 0x44, s.archery_mastery);
        put_u16(r, 0x46, s.building);
        put_u16(r, 0x48, s.healing);
        put_u16(r, 0x4A, s.identification);
        put_u16(r, 0x4C, s.smithing);
        put_u16(r, 0x4E, s.base_armor);
        put_u16(r, 0x50, s.periodic_damage);
        put_u16(r, 0x52, s.raw_0x52);
        put_u16(r, 0x54, s.dexterity_base);
        put_u16(r, 0x56, s.dexterity_current);
        put_u16(r, 0x58, s.charisma_base);
        put_u16(r, 0x5A, s.charisma_current);
        put_u16(r, 0x5C, s.hp_internal);
        put_u16(r, 0x5E, s.strength_base);
        put_u16(r, 0x60, s.strength_current);
        put_u16(r, 0x62, s.endurance_base);
        put_u16(r, 0x64, s.endurance_current);
        for (slot, id) in e.equipment.iter().enumerate() {
            put_u16(r, 0x66 + slot * 2, *id);
        }
        for (slot, id) in e.inventory.iter().enumerate() {
            put_u16(r, 0x72 + slot * 2, *id);
        }
        for (slot, id) in e.additional_equipment.iter().enumerate() {
            put_u16(r, 0xB2 + slot * 2, *id);
        }
        put_u16(r, 0xBC, s.saved_dexterity);
        put_u16(r, 0xBE, s.saved_charisma);
        put_u16(r, 0xC0, s.saved_strength);
        put_u16(r, 0xC2, s.saved_endurance);
        put_u16(r, 0xCA, s.temp_effect_id);
        put_u16(r, 0xCC, e.special_equipment);
        put_u16(r, 0xCE, s.temp_effect_timer);
        r[0xD0] = e.dialogue_id;
        r[0xD4] = e.name_id;
        r[0xD5] = e.nickname_id;
        r[0xD6] = e.visual_index;
        put_u32(r, 0xD8, e.palette_offset);
    }
    block
}

/// Patch a `Game.N`-shaped WorldObject block from live sources.
pub fn patch_world_objects(mut block: Vec<u8>, sources: &[WorldObjectSource]) -> Vec<u8> {
    for s in sources.iter().take(WORLD_OBJECT_COUNT) {
        let o = s.index * WORLD_OBJECT_STRIDE;
        if o + WORLD_OBJECT_STRIDE > block.len() {
            break;
        }
        let r = &mut block[o..o + WORLD_OBJECT_STRIDE];
        put_i32(r, 0x00, s.t0);
        put_i32(r, 0x04, s.t4);
        put_u16(r, 0x08, s.x);
        put_u16(r, 0x0A, s.y);
        put_u16(r, 0x10, s.money as u16);
        put_u16(r, 0x12, s.loc);
        for (slot, id) in s.items.iter().enumerate() {
            put_u16(r, 0x14 + slot * 2, *id);
        }
    }
    block
}

/// Rebuild the runtime marker pool (`0x785BB4`, 200 x 0x40) from the active
/// sources of a `Game.N`-shaped WorldObject block: each accepted record is a
/// copy with `+0x0E = source index`.
pub fn build_markers(
    world_block: &[u8],
    sources: &[WorldObjectSource],
    active: &[ActiveWorldObject],
) -> Vec<u8> {
    let mut out = vec![0u8; MARKER_LEN];
    for (i, m) in active.iter().take(200).enumerate() {
        let so = m.source * WORLD_OBJECT_STRIDE;
        if so + WORLD_OBJECT_STRIDE > world_block.len() {
            continue;
        }
        let o = i * 0x40;
        out[o..o + 0x40].copy_from_slice(&world_block[so..so + 0x40]);
        // Active-list values are authoritative even if the source was patched.
        put_u16(&mut out, o + 0x08, m.x as u16);
        put_u16(&mut out, o + 0x0A, m.y as u16);
        put_u16(&mut out, o + 0x0C, m.sprite);
        put_u16(&mut out, o + 0x0E, m.source as u16);
        put_u16(&mut out, o + 0x10, m.money as u16);
        for (slot, id) in m.items.iter().take(WORLD_OBJECT_ITEM_SLOTS).enumerate() {
            put_u16(&mut out, o + 0x14 + slot * 2, *id);
        }
        // Keep unused slots clear.
        for slot in m.items.len().min(WORLD_OBJECT_ITEM_SLOTS)..WORLD_OBJECT_ITEM_SLOTS {
            put_u16(&mut out, o + 0x14 + slot * 2, 0);
        }
    }
    let _ = sources;
    out
}

/// Patch a `Game.N`-shaped Settlement block from live settlements and their
/// trade state.
pub fn patch_settlements(
    mut block: Vec<u8>,
    settlements: &[Settlement],
    trades: &[SettlementTrade],
) -> Vec<u8> {
    for (i, s) in settlements.iter().take(SETTLEMENT_COUNT).enumerate() {
        let o = i * SETTLEMENT_STRIDE;
        let r = &mut block[o..o + SETTLEMENT_STRIDE];
        r[0] = s.target_base0;
        r[1] = s.target_base1;
        r[2] = s.group_id;
        r[3] = s.loc_id;
        put_i32(r, 0x10, s.money_pool_10);
        put_u16(r, 0x14, s.progress_14);
        put_i32(r, 0x3FC, s.apprentice_start[0]);
        put_i32(r, 0x400, s.apprentice_start[1]);
        put_i32(r, 0x404, s.apprentice_start[2]);
        r[0x4B2] = s.flags_4b2;
        for (t, target) in s.targets.iter().enumerate() {
            let to = 0x18 + t * 8;
            if to + 8 > SETTLEMENT_STRIDE {
                break;
            }
            r[to] = target.key;
            r[to + 1] = target.state;
            put_u16(r, to + 2, target.prop_index);
            put_u16(r, to + 6, target.timer);
        }
        for (w, wp) in s.waypoints.iter().take(WAYPOINT_MAX).enumerate() {
            let wo = WAYPOINT_OFF + w * WAYPOINT_STRIDE;
            r[wo] = wp.facing;
            r[wo + 1] = wp.dialogue_id;
            put_u16(r, wo + 2, wp.owner);
            put_u16(r, wo + 4, wp.x);
            put_u16(r, wo + 6, wp.y);
        }
    }
    for t in trades.iter().take(SETTLEMENT_COUNT) {
        let o = t.index.min(SETTLEMENT_COUNT - 1) * SETTLEMENT_STRIDE;
        // The producer words are the waypoint owners; write them from the live
        // waypoint table so a runtime `0x159`/`0x14C` install survives export.
        let producers = settlements
            .get(t.index)
            .map(|s| [s.producer(1), s.producer(2), s.producer(3), s.voivode()])
            .unwrap_or([t.prod_role2, t.prod_role3, t.prod_role4, t.prod_role5]);
        let r = &mut block[o..o + SETTLEMENT_STRIDE];
        put_u16(
            r,
            crate::constants::game_layout::TRADE_PROD2_OFF,
            producers[0],
        );
        put_u16(
            r,
            crate::constants::game_layout::TRADE_PROD3_OFF,
            producers[1],
        );
        put_u16(
            r,
            crate::constants::game_layout::TRADE_PROD4_OFF,
            producers[2],
        );
        put_u16(
            r,
            crate::constants::game_layout::TRADE_PROD5_OFF,
            producers[3],
        );
        put_i32(r, crate::constants::game_layout::TRADE_TIMER2_OFF, t.timer2);
        put_i32(r, crate::constants::game_layout::TRADE_TIMER4_OFF, t.timer4);
        put_i32(r, crate::constants::game_layout::TRADE_TIMER3_OFF, t.timer3);
        r[crate::constants::game_layout::TRADE_LAST_SLOT_OFF] = t.last_slot as u8;
        put_u16(r, crate::constants::game_layout::TRADE_QUOTA0_OFF, t.quota0);
        put_u16(r, crate::constants::game_layout::TRADE_QUOTA1_OFF, t.quota1);
        put_u16(r, crate::constants::game_layout::TRADE_QUOTA2_OFF, t.quota2);
        for (slot, id) in t.stock2.iter().enumerate() {
            put_u16(
                r,
                crate::constants::game_layout::TRADE_STOCK2_OFF + slot * 2,
                *id,
            );
        }
        for (slot, id) in t.stock3.iter().enumerate() {
            put_u16(
                r,
                crate::constants::game_layout::TRADE_STOCK3_OFF + slot * 2,
                *id,
            );
        }
        for (slot, id) in t.stock4.iter().enumerate() {
            put_u16(
                r,
                crate::constants::game_layout::TRADE_STOCK4_OFF + slot * 2,
                *id,
            );
        }
    }
    block
}

/// Parse a `KONUNG.SAV` slot-name table (10 A- 50-byte CP866 C strings).
pub fn parse_slot_names(bytes: &[u8]) -> Result<[String; SLOT_COUNT], String> {
    if bytes.len() < SLOT_NAMES_LEN {
        return Err(format!(
            "slot-name table is {} bytes, expected {}",
            bytes.len(),
            SLOT_NAMES_LEN
        ));
    }
    let mut out: [String; SLOT_COUNT] = Default::default();
    for (i, slot) in out.iter_mut().enumerate() {
        let o = i * SLOT_NAME_LEN;
        let rec = &bytes[o..o + SLOT_NAME_LEN];
        let end = rec.iter().position(|&b| b == 0).unwrap_or(SLOT_NAME_LEN);
        *slot = crate::names::decode_cp866(&rec[..end])
            .trim_end()
            .to_string();
    }
    Ok(out)
}

/// Write a `KONUNG.SAV` slot-name table, truncating each name to 49 CP866 bytes
/// plus the NUL terminator.
pub fn write_slot_names(names: &[String; SLOT_COUNT]) -> Vec<u8> {
    let mut out = vec![0u8; SLOT_NAMES_LEN];
    for (i, name) in names.iter().enumerate() {
        let o = i * SLOT_NAME_LEN;
        let encoded = crate::names::encode_cp866(name);
        let n = encoded.len().min(SLOT_NAME_LEN - 1);
        out[o..o + n].copy_from_slice(&encoded[..n]);
        out[o + n] = 0;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::game_layout::{
        ENTITY_OFF, GAME_SIZE, GROUP_OFF, SETTLEMENT_OFF, WORLD_OBJECT_OFF,
    };

    fn synthetic() -> LegacySnapshot {
        let mut s = LegacySnapshot::empty();
        s.world_tick = 12345;
        s.leader_index = 0;
        s.quest_flags[7] = 0x0001_0080;
        s.item_table[0] = 9;
        s.entities[0] = 0x06;
        s.entities[0x33] = 3;
        s.marker_count = 1;
        s.markers[0x0E] = 5;
        s.discovered[3] = 0xAA;
        s
    }

    #[test]
    fn snapshot_round_trips_byte_exactly() {
        let s = synthetic();
        let bytes = s.write();
        assert_eq!(bytes.len(), SNAPSHOT_LEN);
        let p = LegacySnapshot::parse(&bytes).expect("parse");
        assert_eq!(p, s);
        assert_eq!(p.write(), bytes);
    }

    #[test]
    fn truncated_snapshot_is_rejected() {
        assert!(LegacySnapshot::parse(&[0u8; 32]).is_err());
    }

    #[test]
    fn leader_and_blocks_decode() {
        let p = synthetic();
        let leader = p.leader().expect("leader");
        assert_eq!(leader.kind, 6);
        assert_eq!(leader.stats.level, 3);
        let markers = p.markers();
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].source, 5);
    }

    #[test]
    fn slot_names_round_trip_cp866() {
        let mut names: [String; SLOT_COUNT] = Default::default();
        names[0] = "Драконий остров".to_string();
        names[9] = "Save 10".to_string();
        let bytes = write_slot_names(&names);
        assert_eq!(bytes.len(), SLOT_NAMES_LEN);
        let back = parse_slot_names(&bytes).expect("parse names");
        assert_eq!(back[0], "Драконий остров");
        assert_eq!(back[9], "Save 10");
        assert_eq!(back[1], "");
    }

    #[test]
    fn blocks_patch_round_trips_models() {
        let p = synthetic();
        let groups = p.groups();
        let entities = p.entities();
        let settlements = p.settlements();
        let g = patch_groups(p.groups.clone(), &groups);
        let e = patch_entities(p.entities.clone(), &entities);
        let st = patch_settlements(p.settlements.clone(), &settlements, &[]);
        assert_eq!(g, p.groups);
        assert_eq!(e, p.entities);
        assert_eq!(st, p.settlements);
    }

    #[test]
    fn world_object_block_offsets_match_game_n() {
        assert_eq!(GROUP_BLOCK_LEN, 0x1900);
        assert_eq!(WORLD_OBJECT_BLOCK_LEN, 0x7D00);
        assert_eq!(ENTITY_BLOCK_LEN, 0x7D000);
        assert_eq!(SETTLEMENT_BLOCK_LEN, 0x5E24);
        assert_eq!(
            GAME_SIZE,
            SETTLEMENT_OFF + SETTLEMENT_COUNT * SETTLEMENT_STRIDE
        );
        assert_eq!(ENTITY_OFF, 0x2A69A);
        assert_eq!(GROUP_OFF, 0x20000);
        assert_eq!(WORLD_OBJECT_OFF, 0x21900);
    }
}
