//! `types` subsystem of [`crate::population`] (split from `population.rs`).

use super::*;

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


/// Authoritative character/gameplay fields of an `Entity` record, parsed
/// verbatim from `Game.N` (`KONUNG_SOURCE_OF_TRUTH.md` §0 label:
/// **BINARY-LAYOUT VERIFIED** for the offsets). These are the persisted,
/// authored values and are **never** derived, averaged or regenerated for
/// existing entities.
///
/// `raw_0x11` has **UNKNOWN** semantics and is retained as an input to the
/// future population-growth constructor (VA 0x420A8C). `raw_0x52` is the venom
/// value copied into the corpse-loot stinger instance (`FUN_00418480`).
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
    /// `+0x52` u16 - venom value used by corpse loot (`FUN_00418480`).
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
    /// `+0x12` u8 — raw field used by the character screen's overlapping
    /// equipment-slot (storage 0/2) branch (set by character creation; `2` in
    /// the shipped new-game records).
    pub raw_0x12: u8,
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
    /// `+0xB2..+0xBB` — 5 additional equipment item ids (`u16`).
    pub additional_equipment: [u16; 5],
    /// `+0xCC` — special equipment item id (`u16`).
    pub special_equipment: u16,
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


/// Both parsed tables.
pub struct GameTables {
    pub groups: Vec<Group>,
    pub entities: Vec<Entity>,
}


/// One settlement waypoint (`FUN_004168a4` / `FUN_0041182c`).
///
/// 8-byte record: `+0` facing, `+1` unknown, `+2` owning Entity index
/// (`0` = unassigned), `+4` target x, `+6` target y (runtime/Entity grid).
/// Field use is ORIGINAL-CODE VERIFIED; the `+1` byte is UNKNOWN.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Waypoint {
    pub facing: u8,
    pub owner: u16,
    pub x: u16,
    pub y: u16,
}


/// One `Settlement` record (`Game.N`, stride `0x4B5`); `loc_id` is byte `+3`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Settlement {
    pub index: usize,
    pub loc_id: u8,
    pub waypoints: Vec<Waypoint>,
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


/// A group refused to map to a valid Entity range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RangeError {
    StartOutOfRange { start: usize },
    EndOutOfRange { start: usize, count: usize, end: usize },
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
