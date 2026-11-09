//! `types` subsystem of [`crate::population`] (split from `population.rs`).

use super::*;

/// A single group spawn rectangle (§4.3). Offsets differ between rectangles:
/// rectangle A reads `+08/+0C/+10/+14`, rectangle B `+0A/+0E/+12/+16`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Group {
    /// Source group id (table index).
    pub index: usize,
    /// `+0x00` u16 — first Entity index owned by this group.
    pub first_entity: u16,
    /// `+08/+0C/+10/+14` — spawn rectangle A.
    pub rect_a: Rect,
    /// `+0A/+0E/+12/+16` — second rectangle B (role unresolved, §15).
    pub rect_b: Rect,
    /// `+0x02` i32 — signed population-growth accumulator. Negative when the
    /// active count is below the current target; consumed on map entry.
    #[serde(skip)]
    pub growth_timer: i32,
    /// `+0x06` i16 — population-target countdown; when it rolls negative the
    /// target `count_18` grows by one and this is reset from `0x45E31E`.
    #[serde(skip)]
    pub growth_countdown: i16,
    /// `+0x18` u8 — current/population-related count.
    pub count_18: u8,
    /// `+0x19` u8 — capacity / maximum-related count.
    pub capacity: u8,
    /// `+0x1B` u8 — location id this group belongs to.
    pub loc_id: u8,
    /// `+0x1A` u8 — target group id (`FUN_00412C30` writes the party group id;
    /// shipped data uses `0`). Authoring data, rehydrated from `Game.N`.
    #[serde(skip)]
    pub target_group: u8,
    /// `+0x1C` u8 — active contiguous entity count used by original traversal.
    pub entity_count: u8,
    /// `+0x1E` u8 — group archetype/class/flags; `0` marks inactive (§4.2).
    pub class_1e: u8,
    /// Runtime group activity byte `+0x1D` (`FUN_004245B4` wake / quest `0x150`).
    /// Runtime-only; persisted through [`crate::save::SaveExtra`].
    #[serde(skip)]
    pub activity_1d: u8,
    /// Runtime "visited" flag set by quest opcode `0x112`
    /// (`DAT_0067C0A9[group*0x20] = 1`). Runtime-only.
    #[serde(skip)]
    pub visited: bool,
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
    /// `+0xBC` u16 — saved dexterity current (temporary-effect backup).
    pub saved_dexterity: u16,
    /// `+0xBE` u16 — saved charisma current.
    pub saved_charisma: u16,
    /// `+0xC0` u16 — saved strength current.
    pub saved_strength: u16,
    /// `+0xC2` u16 — saved endurance current.
    pub saved_endurance: u16,
    /// `+0xCA` u16 — temporary-effect id / period.
    pub temp_effect_id: u16,
    /// `+0xCE` u16 — temporary-effect timer in logic frames.
    pub temp_effect_timer: u16,
}

/// `Entity[2000]`, stride `0x100` (`KONUNG_SOURCE_OF_TRUTH.md` §5).
///
/// `index` is the source entity id. `x`/`y` are persisted coordinates and are
/// not converted to any other domain. `stats`, `equipment` and `inventory` are
/// the authored gameplay state (source of truth for runtime actors).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Entity {
    /// Source entity id (table index).
    pub index: usize,
    /// `+0x00` u8 — **authored action row** (not a separate "kind"): `0/6` idle,
    /// `1/7` walk, `2/8` attack, `3/9` death, `4/5` special (`FUN_0041435c`).
    /// The live runtime row is [`crate::actors::ActorMotion::action`]; this
    /// parsed value is the initial/authored row and is only a fallback for
    /// locations without a population cache.
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
    /// `+0xD2` u8 — per-actor quest/interface state byte.
    ///
    /// The protagonist's copy is [`crate::character::PlayerCharacter::interface_mode_d2`]
    /// (read by quest opcodes `0x145`/`0x158`/`0x16C`). Class-11 subtype 33
    /// («Кукла») writes `0x13` into this byte on every living actor of every
    /// other group in the current location.
    ///
    /// Runtime/quest state, not authored data: skipped by serde so the save
    /// layout stays compatible, and persisted separately through
    /// [`crate::save::SaveExtra::npc_state_d2`].
    #[serde(skip)]
    pub state_d2: u8,
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
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GameTables {
    pub groups: Vec<Group>,
    pub entities: Vec<Entity>,
}

/// One settlement waypoint (`FUN_004168a4` / `FUN_0041182c`).
///
/// 8-byte record: `+0` facing, `+1` unknown, `+2` owning Entity index
/// (`0` = unassigned), `+4` target x, `+6` target y (runtime/Entity grid).
/// Field use is ORIGINAL-CODE VERIFIED; the `+1` byte is UNKNOWN.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Waypoint {
    pub facing: u8,
    pub owner: u16,
    pub x: u16,
    pub y: u16,
}

/// One `Settlement` record (`Game.N`, stride `0x4B5`); `loc_id` is byte `+3`.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Settlement {
    pub index: usize,
    pub loc_id: u8,
    /// `+0x02` group index woken by `FUN_004245B4`.
    pub group_id: u8,
    /// `+0x14` settlement progress bits (`0x112` ORs `0x15`).
    pub progress_14: u16,
    /// `+0x10` settlement money pool (quest `0x11B` pays it out, `0x16D` zeros it).
    /// Runtime-only; persisted through [`crate::save::SaveExtra`].
    #[serde(skip)]
    pub money_pool_10: i32,
    /// `+0x00` target-list base byte (quest `0x147`/`0x169` index `base0 + 7`).
    #[serde(skip)]
    pub target_base0: u8,
    /// `+0x01` target-list count byte (quest `0x169` loop count).
    #[serde(skip)]
    pub target_base1: u8,
    /// Apprentice training timestamps for producer roles 2/3/4
    /// (`Settlement+0x3FC/+0x400/+0x404`). Quest `0x162` stores the world
    /// frame; `0x123` uses the elapsed time for the role skill gain.
    /// Runtime-only; persisted through [`crate::save::SaveExtra`].
    #[serde(skip)]
    pub apprentice_start: [i32; 3],
    /// `+0x4B2` runtime flags (`0x15` set by the projectile prop branch).
    pub flags_4b2: u8,
    /// `+0x18` target list, 8 bytes per entry.
    pub targets: Vec<SettlementTarget>,
    pub waypoints: Vec<Waypoint>,
}

/// One `Settlement+0x18` target entry: `+0x00 key`, `+0x01 state`,
/// `+0x02 u16 prop index`, `+0x06 u16 timer`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SettlementTarget {
    pub key: u8,
    pub state: u8,
    pub prop_index: u16,
    pub timer: u16,
}

/// Runtime trade/restock state for one settlement.
///
/// Stock arrays and production counters share the `Settlement` record:
/// role 2 (`+0x412`, 24), role 3 (`+0x444`, 32), role 4 (`+0x484`, 23);
/// producer words at `+0x3DA/+0x3E2/+0x3EA/+0x3F2` (which **are**
/// `waypoint[1..4].owner`; use [`Settlement::producer`] for gameplay);
/// timers at `+0x4/+0x8/+0xC` (role 3 has no producer, so `timer3`/`+0xC` is
/// the voivode training timer); role-4 pending slot `+0x4B4`. Kept separate
/// from [`Settlement`] so synthetic test fixtures and the AI parser stay
/// unchanged.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SettlementTrade {
    pub index: usize,
    pub loc_id: u8,
    pub flags_4b2: u8,
    pub stock2: [u16; 24],
    pub stock3: [u16; 32],
    pub stock4: [u16; 23],
    pub prod_role2: u16,
    pub prod_role3: u16,
    pub prod_role4: u16,
    pub prod_role5: u16,
    pub timer2: i32,
    pub timer3: i32,
    pub timer4: i32,
    pub last_slot: i8,
    pub quota0: u16,
    pub quota1: u16,
    pub quota2: u16,
}

impl Default for SettlementTrade {
    fn default() -> Self {
        SettlementTrade {
            index: 0,
            loc_id: 0,
            flags_4b2: 0,
            stock2: [0; 24],
            stock3: [0; 32],
            stock4: [0; 23],
            prod_role2: 0,
            prod_role3: 0,
            prod_role4: 0,
            prod_role5: 0,
            timer2: 0,
            timer3: 0,
            timer4: 0,
            last_slot: -1,
            quota0: 0,
            quota1: 0,
            quota2: 0,
        }
    }
}

impl SettlementTrade {
    /// Role 2..=4 stock list as a slice.
    pub fn stock(&self, role: u8) -> Option<&[u16]> {
        match role {
            2 => Some(&self.stock2),
            3 => Some(&self.stock3),
            4 => Some(&self.stock4),
            _ => None,
        }
    }

    /// Mutable role 2..=4 stock list.
    pub fn stock_mut(&mut self, role: u8) -> Option<&mut [u16]> {
        match role {
            2 => Some(&mut self.stock2),
            3 => Some(&mut self.stock3),
            4 => Some(&mut self.stock4),
            _ => None,
        }
    }
}

impl Settlement {
    /// Waypoint owner slot (`1..=5`) for `entity_index`, or `0` when the NPC
    /// owns none of the first five settlement waypoints (`FUN_0041182C`).
    pub fn role_of(&self, entity_index: u16) -> u8 {
        self.waypoints
            .iter()
            .take(5)
            .position(|w| w.owner == entity_index)
            .map(|slot| slot as u8 + 1)
            .unwrap_or(0)
    }

    /// Owner of waypoint `slot` (`0..=4`), or `0` when the slot is unowned.
    ///
    /// The original settlement record has no separate producer fields: the
    /// "producer role" words `+0x3DA/+0x3E2/+0x3EA/+0x3F2` **are**
    /// `waypoint[1..4].owner` (`FUN_0041182C`/`FUN_004168A4`). Use this helper
    /// instead of the mirrored [`SettlementTrade`] fields for gameplay.
    pub fn producer(&self, slot: usize) -> u16 {
        self.waypoints.get(slot).map(|w| w.owner).unwrap_or(0)
    }

    /// `Settlement+0x3F2` — the voivode (`waypoint[4].owner`) who runs the
    /// barracks training pass (`FUN_00417F84`, quest predicate `0x130`).
    pub fn voivode(&self) -> u16 {
        self.producer(4)
    }
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
    StartOutOfRange {
        start: usize,
    },
    EndOutOfRange {
        start: usize,
        count: usize,
        end: usize,
    },
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
