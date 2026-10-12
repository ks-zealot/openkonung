//! Typed parser for the `Game.N` save-state Group/Entity tables.
//!
//! Layout (`KONUNG_SOURCE_OF_TRUTH.md` §3–§5), BINARY-LAYOUT VERIFIED:
//!
//! ```text
//! offset    count   stride   table
//! 0x20000   200     0x20     Group
//! 0x2A69A   2000    0x100    Entity
//! ```
//!
//! `Entity` records are contiguous per group: `first_entity .. first_entity +
//! entity_count` (§4.1). This module only parses and enumerates; it performs no
//! coordinate conversion, no collision queries and no rendering (Phase B).
//!
//! Coordinate-domain note (§1): `Entity.x/+0x08` and `Entity.y/+0x0A` are
//! persisted Entity/transition coordinates, **not** 320x160 collision-grid
//! indices. They are stored here exactly as persisted and never rescaled.
//!
//! # Population growth (documented, **not implemented**)
//!
//! When the original game creates a **genuinely new** group member beyond the
//! current `entity_count` (constructor VA `0x420A8C`), it averages the existing
//! group members' `dexterity_base`, `charisma_base`, `level`, `money`,
//! `base_armor`, `Entity+0x52`, `strength_base`, `Entity+0x11` and
//! `endurance_base`, assigns those averages to the new Entity, sets
//! `current = averaged base` for the primary stats and `HP = 1600`, copies
//! `visual_index`/`palette_offset` from a randomly-chosen existing member,
//! clears equipment/inventory, sets name/nickname to 0 and dialogue to `0xFF`.
//!
//! Existing entities (including authored `(0,0)` records) are **never** grown or
//! regenerated; their parsed stats are the source of truth.

use std::fmt;
use std::ops::Range;

use crate::res::{i32v, u16, u32};

/// Exact `Game.N` size (matches `edges::GAME_SIZE`).
pub const GAME_SIZE: usize = 0xAD4BE;

/// Group table offset/stride/count (`KONUNG_SOURCE_OF_TRUTH.md` §3).
pub const GROUP_OFF: usize = 0x20000;
pub const GROUP_STRIDE: usize = 0x20;
pub const GROUP_COUNT: usize = 200;

/// Entity table offset/stride/count (`KONUNG_SOURCE_OF_TRUTH.md` §3/§5).
///
/// `ENTITY_OFF` is intentionally unaligned (`0x2A69A`); reads go through the
/// byte-wise `res::{u16,u32}` helpers.
pub const ENTITY_OFF: usize = 0x2A69A;
pub const ENTITY_STRIDE: usize = 0x100;
pub const ENTITY_COUNT: usize = 2000;

/// One past the last byte of the Entity table.
pub const ENTITY_END: usize = ENTITY_OFF + ENTITY_COUNT * ENTITY_STRIDE;

/// A single group spawn rectangle (§4.3). Offsets differ between rectangles:
/// rectangle A reads `+08/+0C/+10/+14`, rectangle B `+0A/+0E/+12/+16`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub x1: u16,
    pub x2: u16,
    pub y1: u16,
    pub y2: u16,
}

/// `Group[200]`, stride `0x20` (`KONUNG_SOURCE_OF_TRUTH.md` §4).
///
/// `index` is the source group id and is preserved. The record is interleaved;
/// rectangle A/B are decoded from their own offsets, never as one rectangle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    /// Source group id (table index).
    pub index: usize,
    /// `+0x00` u16 — first Entity index owned by this group.
    pub first_entity: u16,
    /// `+08/+0C/+10/+14` — spawn rectangle A.
    pub rect_a: Rect,
    /// `+0A/+0E/+12/+16` — second rectangle B (role unresolved, §15).
    pub rect_b: Rect,
    /// `+0x18` u8 — current/population-related count.
    pub count_18: u8,
    /// `+0x19` u8 — capacity / maximum-related count.
    pub capacity: u8,
    /// `+0x1B` u8 — location id this group belongs to.
    pub loc_id: u8,
    /// `+0x1C` u8 — active contiguous entity count used by original traversal.
    pub entity_count: u8,
    /// `+0x1E` u8 — group archetype/class/flags; `0` marks inactive (§4.2).
    pub class_1e: u8,
}

impl Group {
    /// Group 0 is reserved/special in the original setup path, which iterates
    /// from index 1 (`KONUNG_SOURCE_OF_TRUTH.md` §4.2). Preserved as a flag
    /// rather than silently filtered.
    pub fn is_reserved_group_zero(&self) -> bool {
        self.index == 0
    }

    /// `+0x1E != 0` — the original setup path skips zero-class groups (§4.2).
    pub fn is_active_class(&self) -> bool {
        self.class_1e != 0
    }

    /// Contiguous Entity index range `first_entity .. first_entity+entity_count`,
    /// validated against the 2000-entry Entity table (§4.1).
    pub fn entity_range(&self) -> Result<Range<usize>, RangeError> {
        let start = self.first_entity as usize;
        let count = self.entity_count as usize;
        if start > ENTITY_COUNT {
            return Err(RangeError::StartOutOfRange { start });
        }
        let end = start + count;
        if end > ENTITY_COUNT {
            return Err(RangeError::EndOutOfRange { start, count, end });
        }
        Ok(start..end)
    }
}

/// Authoritative character/gameplay fields of an `Entity` record, parsed
/// verbatim from `Game.N` (`KONUNG_SOURCE_OF_TRUTH.md` §0 label:
/// **BINARY-LAYOUT VERIFIED** for the offsets). These are the persisted,
/// authored values and are **never** derived, averaged or regenerated for
/// existing entities.
///
/// `raw_0x11` and `raw_0x52` have **UNKNOWN** semantics and are retained only as
/// inputs to the future population-growth constructor (VA 0x420A8C, §6).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EntityStats {
    /// `+0x11` u8 — raw field, semantics UNKNOWN (growth-constructor input).
    pub raw_0x11: u8,
    /// `+0x33` u8 — level.
    pub level: u8,
    /// `+0x34` i32 — money.
    pub money: i32,
    /// `+0x38` u32 — available/free XP.
    pub available_xp: u32,
    /// `+0x3C` u32 — invested XP.
    pub invested_xp: u32,
    /// `+0x40` u16 — melee mastery.
    pub melee_mastery: u16,
    /// `+0x42` u16 — trade.
    pub trade: u16,
    /// `+0x44` u16 — archery mastery.
    pub archery_mastery: u16,
    /// `+0x46` u16 — building.
    pub building: u16,
    /// `+0x48` u16 — healing.
    pub healing: u16,
    /// `+0x4A` u16 — identification.
    pub identification: u16,
    /// `+0x4C` u16 — smithing.
    pub smithing: u16,
    /// `+0x4E` u16 — base armor.
    pub base_armor: u16,
    /// `+0x50` u16 — periodic damage.
    pub periodic_damage: u16,
    /// `+0x52` u16 — raw field, semantics UNKNOWN (growth-constructor input).
    pub raw_0x52: u16,
    /// `+0x54` u16 — dexterity base.
    pub dexterity_base: u16,
    /// `+0x56` u16 — dexterity current.
    pub dexterity_current: u16,
    /// `+0x58` u16 — charisma base.
    pub charisma_base: u16,
    /// `+0x5A` u16 — charisma current.
    pub charisma_current: u16,
    /// `+0x5C` u16 — HP (internal representation).
    pub hp_internal: u16,
    /// `+0x5E` u16 — strength base.
    pub strength_base: u16,
    /// `+0x60` u16 — strength current.
    pub strength_current: u16,
    /// `+0x62` u16 — endurance base.
    pub endurance_base: u16,
    /// `+0x64` u16 — endurance current.
    pub endurance_current: u16,
}

/// `Entity[2000]`, stride `0x100` (`KONUNG_SOURCE_OF_TRUTH.md` §5).
///
/// `index` is the source entity id. `x`/`y` are persisted coordinates and are
/// not converted to any other domain. `stats`, `equipment` and `inventory` are
/// the authored gameplay state (source of truth for runtime actors).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entity {
    /// Source entity id (table index).
    pub index: usize,
    /// `+0x00` u8.
    pub kind: u8,
    /// `+0x01` u8 — facing/direction.
    pub facing: u8,
    /// `+0x02` u8.
    pub flags_a: u8,
    /// `+0x03` u8.
    pub flags_b: u8,
    /// `+0x04` u8 — archetype/class flags; renderer family uses bit 0x40 (§5.1).
    pub archetype: u8,
    /// `+0x05` u8 — owning group id.
    pub group_id: u8,
    /// `+0x08` u16 — persisted x (NOT a collision-grid index, §1).
    pub x: u16,
    /// `+0x0A` u16 — persisted y (NOT a collision-grid index, §1).
    pub y: u16,
    /// `+0x33..+0x64` — character/gameplay stats (authored, never regenerated).
    pub stats: EntityStats,
    /// `+0x66..+0x71` — six equipment item ids (`u16`); not interpreted yet.
    pub equipment: [u16; 6],
    /// `+0x72..+0xB1` — 32 inventory item ids (`u16`); not interpreted yet.
    pub inventory: [u16; 32],
    /// `+0xD0` u8 — dialogue id; 0xFF = none in known path.
    pub dialogue_id: u8,
    /// `+0xD4` u8 — name id.
    pub name_id: u8,
    /// `+0xD5` u8 — nickname id.
    pub nickname_id: u8,
    /// `+0xD6` u8 — family-local visual index.
    pub visual_index: u8,
    /// `+0xD8` u32 — palette/palette-base offset.
    pub palette_offset: u32,
}

impl Entity {
    /// Diagnostic player-style state block, keyed by source entity id (task §7).
    /// Prints the authored values verbatim; no derivation.
    pub fn debug_block(&self) -> String {
        let s = &self.stats;
        let equip: Vec<String> = self.equipment.iter().map(|v| v.to_string()).collect();
        let inv: Vec<String> = self.inventory.iter().map(|v| v.to_string()).collect();
        format!(
            "Entity {}\ngroup={}\npos=({},{})\nclass={}\nlevel={}\n\nHP={}\nmoney={}\n\nDEX={}/{}\nCHA={}/{}\nSTR={}/{}\nEND={}/{}\n\nmelee={}\narchery={}\ntrade={}\nbuilding={}\nhealing={}\nidentification={}\nsmithing={}\n\nequipment=[{}]\ninventory=[{}]\n",
            self.index,
            self.group_id,
            self.x,
            self.y,
            self.archetype,
            s.level,
            s.hp_internal,
            s.money,
            s.dexterity_base,
            s.dexterity_current,
            s.charisma_base,
            s.charisma_current,
            s.strength_base,
            s.strength_current,
            s.endurance_base,
            s.endurance_current,
            s.melee_mastery,
            s.archery_mastery,
            s.trade,
            s.building,
            s.healing,
            s.identification,
            s.smithing,
            equip.join(", "),
            inv.join(", "),
        )
    }
}

/// Both parsed tables.
pub struct GameTables {
    pub groups: Vec<Group>,
    pub entities: Vec<Entity>,
}

impl GameTables {
    /// Entity by source id, if the id is inside the 2000-entry table.
    pub fn entity(&self, index: usize) -> Option<&Entity> {
        self.entities.get(index)
    }

    /// Group by source id, if the id is inside the 200-entry table.
    pub fn group(&self, index: usize) -> Option<&Group> {
        self.groups.get(index)
    }
}

/// A group refused to map to a valid Entity range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RangeError {
    StartOutOfRange { start: usize },
    EndOutOfRange { start: usize, count: usize, end: usize },
}

impl fmt::Display for RangeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RangeError::StartOutOfRange { start } => write!(
                f,
                "group first_entity {start} is outside the 0..{ENTITY_COUNT} Entity table"
            ),
            RangeError::EndOutOfRange { start, count, end } => write!(
                f,
                "group entity range {start}..{end} (first {start} + count {count}) exceeds the {ENTITY_COUNT}-entry Entity table"
            ),
        }
    }
}

impl std::error::Error for RangeError {}

/// Parse the Group table. Fails loudly on a truncated file.
pub fn parse_groups(data: &[u8]) -> Result<Vec<Group>, String> {
    let required = GROUP_OFF + GROUP_COUNT * GROUP_STRIDE;
    if data.len() < required {
        return Err(format!(
            "Group table truncated: file is {} bytes, need at least {} (0x{:X})",
            data.len(),
            required,
            required
        ));
    }
    let mut groups = Vec::with_capacity(GROUP_COUNT);
    for index in 0..GROUP_COUNT {
        let o = GROUP_OFF + index * GROUP_STRIDE;
        let r = &data[o..o + GROUP_STRIDE];
        groups.push(Group {
            index,
            first_entity: u16(r, 0x00) as u16,
            rect_a: Rect {
                x1: u16(r, 0x08) as u16,
                x2: u16(r, 0x0C) as u16,
                y1: u16(r, 0x10) as u16,
                y2: u16(r, 0x14) as u16,
            },
            rect_b: Rect {
                x1: u16(r, 0x0A) as u16,
                x2: u16(r, 0x0E) as u16,
                y1: u16(r, 0x12) as u16,
                y2: u16(r, 0x16) as u16,
            },
            count_18: r[0x18],
            capacity: r[0x19],
            loc_id: r[0x1B],
            entity_count: r[0x1C],
            class_1e: r[0x1E],
        });
    }
    Ok(groups)
}

/// Parse the Entity table. Fails loudly on a truncated file.
pub fn parse_entities(data: &[u8]) -> Result<Vec<Entity>, String> {
    if data.len() < ENTITY_END {
        return Err(format!(
            "Entity table truncated: file is {} bytes, need at least {} (0x{:X})",
            data.len(),
            ENTITY_END,
            ENTITY_END
        ));
    }
    let mut entities = Vec::with_capacity(ENTITY_COUNT);
    for index in 0..ENTITY_COUNT {
        let o = ENTITY_OFF + index * ENTITY_STRIDE;
        let b = &data[o..o + ENTITY_STRIDE];
        let stats = EntityStats {
            raw_0x11: b[0x11],
            level: b[0x33],
            money: i32v(b, 0x34),
            available_xp: u32(b, 0x38),
            invested_xp: u32(b, 0x3C),
            melee_mastery: u16(b, 0x40) as u16,
            trade: u16(b, 0x42) as u16,
            archery_mastery: u16(b, 0x44) as u16,
            building: u16(b, 0x46) as u16,
            healing: u16(b, 0x48) as u16,
            identification: u16(b, 0x4A) as u16,
            smithing: u16(b, 0x4C) as u16,
            base_armor: u16(b, 0x4E) as u16,
            periodic_damage: u16(b, 0x50) as u16,
            raw_0x52: u16(b, 0x52) as u16,
            dexterity_base: u16(b, 0x54) as u16,
            dexterity_current: u16(b, 0x56) as u16,
            charisma_base: u16(b, 0x58) as u16,
            charisma_current: u16(b, 0x5A) as u16,
            hp_internal: u16(b, 0x5C) as u16,
            strength_base: u16(b, 0x5E) as u16,
            strength_current: u16(b, 0x60) as u16,
            endurance_base: u16(b, 0x62) as u16,
            endurance_current: u16(b, 0x64) as u16,
        };
        let mut equipment = [0u16; 6];
        for (i, slot) in equipment.iter_mut().enumerate() {
            *slot = u16(b, 0x66 + i * 2) as u16;
        }
        let mut inventory = [0u16; 32];
        for (i, slot) in inventory.iter_mut().enumerate() {
            *slot = u16(b, 0x72 + i * 2) as u16;
        }
        entities.push(Entity {
            index,
            kind: b[0x00],
            facing: b[0x01],
            flags_a: b[0x02],
            flags_b: b[0x03],
            archetype: b[0x04],
            group_id: b[0x05],
            x: u16(b, 0x08) as u16,
            y: u16(b, 0x0A) as u16,
            stats,
            equipment,
            inventory,
            dialogue_id: b[0xD0],
            name_id: b[0xD4],
            nickname_id: b[0xD5],
            visual_index: b[0xD6],
            palette_offset: u32(b, 0xD8),
        });
    }
    Ok(entities)
}

/// Parse both Group and Entity tables.
pub fn parse_tables(data: &[u8]) -> Result<GameTables, String> {
    let groups = parse_groups(data)?;
    let entities = parse_entities(data)?;
    Ok(GameTables { groups, entities })
}

/// Diagnostic report for one group selected for a location.
#[derive(Clone, Debug)]
pub struct GroupReport {
    pub group: Group,
    pub reserved_group_zero: bool,
    pub valid_class: bool,
    /// Validated `first_entity .. first_entity+entity_count`, or the reason the
    /// range is invalid (in which case `entities` is empty — never clipped).
    pub range: Result<Range<usize>, RangeError>,
    pub entities: Vec<Entity>,
}

/// Enumerate every group whose `loc_id` matches, preserving source group/entity
/// ids and listing every entity in `first_entity .. first_entity+entity_count`.
///
/// This is diagnostic: group 0 (`reserved_group_zero`) and `+0x1E == 0`
/// (`valid_class == false`) groups are reported, not silently dropped. Original
/// setup filtering is available separately via [`active_entities`].
pub fn diagnose_location(tables: &GameTables, loc_id: u8) -> Vec<GroupReport> {
    let mut out = Vec::new();
    for g in tables.groups.iter().filter(|g| g.loc_id == loc_id) {
        let range = g.entity_range();
        let entities = match &range {
            Ok(r) => tables.entities[r.clone()].to_vec(),
            Err(_) => Vec::new(),
        };
        out.push(GroupReport {
            group: g.clone(),
            reserved_group_zero: g.is_reserved_group_zero(),
            valid_class: g.is_active_class(),
            range,
            entities,
        });
    }
    out
}

/// Entities selected by the original location setup semantics: group index >= 1
/// and `Group +0x1E != 0` (`KONUNG_SOURCE_OF_TRUTH.md` §4.2). Group 0 is
/// reserved and excluded. Invalid ranges contribute nothing.
pub fn active_entities(tables: &GameTables, loc_id: u8) -> Vec<Entity> {
    let mut out = Vec::new();
    for g in tables
        .groups
        .iter()
        .filter(|g| g.index >= 1 && g.loc_id == loc_id && g.is_active_class())
    {
        if let Ok(r) = g.entity_range() {
            out.extend_from_slice(&tables.entities[r]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_game() -> Vec<u8> {
        vec![0u8; ENTITY_END]
    }

    fn set_u16(data: &mut [u8], o: usize, v: u16) {
        data[o] = (v & 0xFF) as u8;
        data[o + 1] = (v >> 8) as u8;
    }

    fn set_u32(data: &mut [u8], o: usize, v: u32) {
        data[o] = (v & 0xFF) as u8;
        data[o + 1] = ((v >> 8) & 0xFF) as u8;
        data[o + 2] = ((v >> 16) & 0xFF) as u8;
        data[o + 3] = ((v >> 24) & 0xFF) as u8;
    }

    fn write_group(data: &mut [u8], index: usize, first: u16, count: u8, loc: u8, class: u8) {
        let o = GROUP_OFF + index * GROUP_STRIDE;
        set_u16(data, o, first);
        data[o + 0x1B] = loc;
        data[o + 0x1C] = count;
        data[o + 0x1E] = class;
    }

    fn write_entity(data: &mut [u8], index: usize, kind: u8, grp: u8, x: u16, y: u16) {
        let o = ENTITY_OFF + index * ENTITY_STRIDE;
        data[o] = kind;
        data[o + 0x05] = grp;
        set_u16(data, o + 0x08, x);
        set_u16(data, o + 0x0A, y);
    }

    #[test]
    fn group_field_offsets_are_exact() {
        let mut data = empty_game();
        let o = GROUP_OFF + 5 * GROUP_STRIDE;
        set_u16(&mut data, o + 0x00, 0x1234);
        set_u16(&mut data, o + 0x08, 0xA001);
        set_u16(&mut data, o + 0x0A, 0xB001);
        set_u16(&mut data, o + 0x0C, 0xA002);
        set_u16(&mut data, o + 0x0E, 0xB002);
        set_u16(&mut data, o + 0x10, 0xA003);
        set_u16(&mut data, o + 0x12, 0xB003);
        set_u16(&mut data, o + 0x14, 0xA004);
        set_u16(&mut data, o + 0x16, 0xB004);
        data[o + 0x18] = 0x18;
        data[o + 0x19] = 0x19;
        data[o + 0x1B] = 0x1B;
        data[o + 0x1C] = 0x1C;
        data[o + 0x1E] = 0x1E;

        let groups = parse_groups(&data).unwrap();
        let g = &groups[5];
        assert_eq!(g.index, 5);
        assert_eq!(g.first_entity, 0x1234);
        assert_eq!(g.rect_a, Rect { x1: 0xA001, x2: 0xA002, y1: 0xA003, y2: 0xA004 });
        assert_eq!(g.rect_b, Rect { x1: 0xB001, x2: 0xB002, y1: 0xB003, y2: 0xB004 });
        assert_eq!(g.count_18, 0x18);
        assert_eq!(g.capacity, 0x19);
        assert_eq!(g.loc_id, 0x1B);
        assert_eq!(g.entity_count, 0x1C);
        assert_eq!(g.class_1e, 0x1E);
    }

    #[test]
    fn entity_field_offsets_are_exact_including_unaligned_base() {
        let mut data = empty_game();
        let o = ENTITY_OFF + 7 * ENTITY_STRIDE;
        data[o + 0x00] = 0x01;
        data[o + 0x01] = 0x02;
        data[o + 0x02] = 0x03;
        data[o + 0x03] = 0x04;
        data[o + 0x04] = 0x05;
        data[o + 0x05] = 0x06;
        set_u16(&mut data, o + 0x08, 0x0A0B);
        set_u16(&mut data, o + 0x0A, 0x0C0D);
        data[o + 0xD0] = 0xD0;
        data[o + 0xD4] = 0xD4;
        data[o + 0xD5] = 0xD5;
        data[o + 0xD6] = 0xD6;
        set_u32(&mut data, o + 0xD8, 0xDEAD_BEEF);

        let entities = parse_entities(&data).unwrap();
        let e = &entities[7];
        assert_eq!(e.index, 7);
        assert_eq!(e.kind, 0x01);
        assert_eq!(e.facing, 0x02);
        assert_eq!(e.flags_a, 0x03);
        assert_eq!(e.flags_b, 0x04);
        assert_eq!(e.archetype, 0x05);
        assert_eq!(e.group_id, 0x06);
        assert_eq!(e.x, 0x0A0B);
        assert_eq!(e.y, 0x0C0D);
        assert_eq!(e.dialogue_id, 0xD0);
        assert_eq!(e.name_id, 0xD4);
        assert_eq!(e.nickname_id, 0xD5);
        assert_eq!(e.visual_index, 0xD6);
        assert_eq!(e.palette_offset, 0xDEAD_BEEF);
    }

    #[test]
    fn entity_range_boundaries() {
        // Empty range at the very end is valid.
        let g = Group {
            index: 1,
            first_entity: ENTITY_COUNT as u16,
            entity_count: 0,
            ..empty_group(1)
        };
        assert_eq!(g.entity_range().unwrap(), ENTITY_COUNT..ENTITY_COUNT);

        // Last valid range.
        let g = Group { first_entity: 1999, entity_count: 1, ..empty_group(1) };
        assert_eq!(g.entity_range().unwrap(), 1999..2000);

        // One past the end is not.
        let g = Group { first_entity: 1999, entity_count: 2, ..empty_group(1) };
        assert!(matches!(g.entity_range(), Err(RangeError::EndOutOfRange { .. })));

        // Start beyond the table.
        let g = Group { first_entity: 2001, entity_count: 0, ..empty_group(1) };
        assert!(matches!(g.entity_range(), Err(RangeError::StartOutOfRange { .. })));
    }

    fn empty_group(index: usize) -> Group {
        Group {
            index,
            first_entity: 0,
            rect_a: Rect::default(),
            rect_b: Rect::default(),
            count_18: 0,
            capacity: 0,
            loc_id: 0,
            entity_count: 0,
            class_1e: 0,
        }
    }

    #[test]
    fn diagnose_filters_location_and_preserves_ids() {
        let mut data = empty_game();
        // Group 3 -> entities 10..12 on location 1.
        write_group(&mut data, 3, 10, 2, 1, 79);
        write_entity(&mut data, 10, 0x11, 3, 100, 60);
        write_entity(&mut data, 11, 0x12, 3, 101, 61);
        // Another group on a different location.
        write_group(&mut data, 4, 20, 1, 2, 65);
        write_entity(&mut data, 20, 0x21, 4, 7, 8);

        let tables = parse_tables(&data).unwrap();
        let reports = diagnose_location(&tables, 1);
        assert_eq!(reports.len(), 1);
        let r = &reports[0];
        assert_eq!(r.group.index, 3);
        assert_eq!(r.entities.len(), 2);
        assert_eq!(r.entities[0].index, 10);
        assert_eq!(r.entities[1].index, 11);
        assert_eq!(r.entities[0].group_id, 3);
        assert_eq!((r.entities[0].x, r.entities[0].y), (100, 60));
        assert!(!r.reserved_group_zero);
        assert!(r.valid_class);
    }

    #[test]
    fn diagnose_reports_invalid_range_without_clipping() {
        let mut data = empty_game();
        // Group 5 claims 3000..3003, beyond the 2000-entry table.
        write_group(&mut data, 5, 3000, 3, 1, 65);

        let tables = parse_tables(&data).unwrap();
        let reports = diagnose_location(&tables, 1);
        assert_eq!(reports.len(), 1);
        assert!(reports[0].range.is_err());
        assert!(reports[0].entities.is_empty());
    }

    #[test]
    fn reserved_group_zero_is_flagged() {
        let mut data = empty_game();
        write_group(&mut data, 0, 0, 1, 1, 32);
        write_entity(&mut data, 0, 0x06, 0, 100, 60);

        let tables = parse_tables(&data).unwrap();
        let reports = diagnose_location(&tables, 1);
        assert_eq!(reports.len(), 1);
        assert!(reports[0].reserved_group_zero);
        assert_eq!(reports[0].entities.len(), 1);
    }

    #[test]
    fn active_entities_excludes_group_zero_and_inactive_class() {
        let mut data = empty_game();
        write_group(&mut data, 0, 0, 1, 1, 32); // reserved
        write_entity(&mut data, 0, 0x06, 0, 100, 60);
        write_group(&mut data, 1, 10, 1, 1, 0); // inactive class
        write_entity(&mut data, 10, 0x11, 1, 1, 1);
        write_group(&mut data, 2, 20, 1, 1, 65); // active
        write_entity(&mut data, 20, 0x12, 2, 2, 2);

        let tables = parse_tables(&data).unwrap();
        let active = active_entities(&tables, 1);
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].index, 20);
        assert_eq!(active[0].group_id, 2);
    }

    #[test]
    fn truncated_buffers_error() {
        let short = vec![0u8; 16];
        assert!(parse_groups(&short).is_err());
        assert!(parse_entities(&short).is_err());
        assert!(parse_tables(&short).is_err());

        // Long enough for Group but not Entity.
        let mid = vec![0u8; ENTITY_OFF];
        assert!(parse_groups(&mid).is_ok());
        assert!(parse_entities(&mid).is_err());
    }

    #[test]
    fn last_entity_is_readable() {
        let mut data = empty_game();
        set_u16(&mut data, ENTITY_OFF + 1999 * ENTITY_STRIDE + 0x08, 0xBEEF);
        set_u32(&mut data, ENTITY_OFF + 1999 * ENTITY_STRIDE + 0xD8, 0xCAFE_F00D);
        let entities = parse_entities(&data).unwrap();
        let e = &entities[1999];
        assert_eq!(e.x, 0xBEEF);
        assert_eq!(e.palette_offset, 0xCAFE_F00D);
    }

    #[test]
    fn entity_stats_equipment_inventory_offsets() {
        let mut data = empty_game();
        let o = ENTITY_OFF + 3 * ENTITY_STRIDE;
        data[o + 0x11] = 0x11;
        data[o + 0x33] = 7;
        set_u32(&mut data, o + 0x34, 1234); // money i32
        set_u32(&mut data, o + 0x38, 55);
        set_u32(&mut data, o + 0x3C, 66);
        set_u16(&mut data, o + 0x40, 100);
        set_u16(&mut data, o + 0x42, 32);
        set_u16(&mut data, o + 0x44, 100);
        set_u16(&mut data, o + 0x46, 1);
        set_u16(&mut data, o + 0x48, 50);
        set_u16(&mut data, o + 0x4A, 20);
        set_u16(&mut data, o + 0x4C, 10);
        set_u16(&mut data, o + 0x4E, 10);
        set_u16(&mut data, o + 0x50, 5);
        set_u16(&mut data, o + 0x52, 0x52);
        set_u16(&mut data, o + 0x54, 10);
        set_u16(&mut data, o + 0x56, 10);
        set_u16(&mut data, o + 0x58, 10);
        set_u16(&mut data, o + 0x5A, 10);
        set_u16(&mut data, o + 0x5C, 1600);
        set_u16(&mut data, o + 0x5E, 15);
        set_u16(&mut data, o + 0x60, 15);
        set_u16(&mut data, o + 0x62, 10);
        set_u16(&mut data, o + 0x64, 10);
        for i in 0..6 {
            set_u16(&mut data, o + 0x66 + i * 2, 200 + i as u16);
        }
        for i in 0..32 {
            set_u16(&mut data, o + 0x72 + i * 2, 300 + i as u16);
        }

        let entities = parse_entities(&data).unwrap();
        let e = &entities[3];
        let s = &e.stats;
        assert_eq!(s.raw_0x11, 0x11);
        assert_eq!(s.level, 7);
        assert_eq!(s.money, 1234);
        assert_eq!((s.available_xp, s.invested_xp), (55, 66));
        assert_eq!((s.melee_mastery, s.trade, s.archery_mastery), (100, 32, 100));
        assert_eq!((s.building, s.healing, s.identification, s.smithing), (1, 50, 20, 10));
        assert_eq!((s.base_armor, s.periodic_damage, s.raw_0x52), (10, 5, 0x52));
        assert_eq!((s.dexterity_base, s.dexterity_current), (10, 10));
        assert_eq!((s.charisma_base, s.charisma_current), (10, 10));
        assert_eq!((s.strength_base, s.strength_current), (15, 15));
        assert_eq!((s.endurance_base, s.endurance_current), (10, 10));
        assert_eq!(s.hp_internal, 1600);
        assert_eq!(e.equipment, [200, 201, 202, 203, 204, 205]);
        assert_eq!(e.inventory[0], 300);
        assert_eq!(e.inventory[31], 331);
    }

    fn game1() -> Option<Vec<u8>> {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../Game.1");
        std::fs::read(&p).ok()
    }

    #[test]
    fn real_game1_entity_277_and_539_regression() {
        let Some(data) = game1() else {
            eprintln!("skipping: Game.1 not present");
            return;
        };
        let t = parse_tables(&data).unwrap();

        let e = &t.entities[277];
        assert_eq!(e.group_id, 23);
        assert_eq!((e.x, e.y), (74, 39));
        assert_eq!(e.archetype, 5);
        assert_eq!(e.stats.level, 1);
        assert_eq!((e.stats.dexterity_base, e.stats.dexterity_current), (10, 10));
        assert_eq!((e.stats.charisma_base, e.stats.charisma_current), (10, 10));
        assert_eq!((e.stats.strength_base, e.stats.strength_current), (15, 15));
        assert_eq!((e.stats.endurance_base, e.stats.endurance_current), (10, 10));
        assert_eq!(e.stats.hp_internal, 1600);
        assert_eq!(e.stats.melee_mastery, 100);
        assert_eq!(e.stats.archery_mastery, 100);
        assert_eq!(e.stats.trade, 32);
        assert_eq!(e.equipment, [0, 0, 288, 289, 0, 0]);

        let e = &t.entities[539];
        assert_eq!(e.stats.level, 3);
        assert_eq!((e.stats.dexterity_base, e.stats.dexterity_current), (28, 28));
        assert_eq!((e.stats.charisma_base, e.stats.charisma_current), (25, 25));
        assert_eq!((e.stats.strength_base, e.stats.strength_current), (29, 29));
        assert_eq!((e.stats.endurance_base, e.stats.endurance_current), (25, 25));
        assert_eq!(e.stats.melee_mastery, 700);
        assert_eq!(e.stats.archery_mastery, 700);
        assert_eq!(e.stats.building, 5);
        assert_eq!(e.equipment, [0, 0, 745, 746, 747, 748]);
    }

    #[test]
    fn real_game1_entity_debug_block() {
        let Some(data) = game1() else { return };
        let t = parse_tables(&data).unwrap();
        let b = t.entities[277].debug_block();
        for needle in [
            "Entity 277",
            "group=23",
            "pos=(74,39)",
            "class=5",
            "level=1",
            "HP=1600",
            "DEX=10/10",
            "CHA=10/10",
            "STR=15/15",
            "END=10/10",
            "melee=100",
            "archery=100",
            "trade=32",
        ] {
            assert!(b.contains(needle), "missing {needle:?} in:\n{b}");
        }
    }
}
