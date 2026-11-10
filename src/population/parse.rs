//! `parse` subsystem of [`crate::population`] (split from `population.rs`).

use super::*;

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
        groups.push(parse_group_record(index, &data[o..o + GROUP_STRIDE]));
    }
    Ok(groups)
}

/// Parse one 0x20-byte `Group` record. Shared by `Game.N` and legacy snapshot
/// blocks (both use the runtime layout).
pub fn parse_group_record(index: usize, r: &[u8]) -> Group {
    Group {
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
        growth_timer: i32v(r, 0x02),
        growth_countdown: (u16(r, 0x06) as u16) as i16,
        count_18: r[0x18],
        capacity: r[0x19],
        loc_id: r[0x1B],
        target_group: r[0x1A],
        entity_count: r[0x1C],
        class_1e: r[0x1E],
        activity_1d: r[0x1D],
        visited: false,
    }
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
        entities.push(parse_entity_record(index, &data[o..o + ENTITY_STRIDE]));
    }
    Ok(entities)
}

/// Parse one 0x100-byte `Entity` record. Shared by `Game.N` and legacy snapshot
/// blocks; both store the runtime `Entity` layout.
pub fn parse_entity_record(index: usize, b: &[u8]) -> Entity {
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
        saved_dexterity: u16(b, 0xBC) as u16,
        saved_charisma: u16(b, 0xBE) as u16,
        saved_strength: u16(b, 0xC0) as u16,
        saved_endurance: u16(b, 0xC2) as u16,
        temp_effect_id: u16(b, 0xCA) as u16,
        temp_effect_timer: u16(b, 0xCE) as u16,
    };
    let mut equipment = [0u16; 6];
    for (i, slot) in equipment.iter_mut().enumerate() {
        *slot = u16(b, 0x66 + i * 2) as u16;
    }
    let mut inventory = [0u16; 32];
    for (i, slot) in inventory.iter_mut().enumerate() {
        *slot = u16(b, 0x72 + i * 2) as u16;
    }
    let mut additional_equipment = [0u16; 5];
    for (i, slot) in additional_equipment.iter_mut().enumerate() {
        *slot = u16(b, 0xB2 + i * 2) as u16;
    }
    Entity {
        index,
        kind: b[0x00],
        facing: b[0x01],
        flags_a: b[0x02],
        flags_b: b[0x03],
        archetype: b[0x04],
        group_id: b[0x05],
        raw_0x12: b[0x12],
        x: u16(b, 0x08) as u16,
        y: u16(b, 0x0A) as u16,
        stats,
        equipment,
        inventory,
        dialogue_id: b[0xD0],
        state_d2: b[0xD2],
        name_id: b[0xD4],
        nickname_id: b[0xD5],
        visual_index: b[0xD6],
        palette_offset: u32(b, 0xD8),
        additional_equipment,
        special_equipment: u16(b, 0xCC) as u16,
    }
}

/// Parse both Group and Entity tables.
pub fn parse_tables(data: &[u8]) -> Result<GameTables, String> {
    let groups = parse_groups(data)?;
    let entities = parse_entities(data)?;
    Ok(GameTables { groups, entities })
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

// --- Settlement / citizen waypoint table (`KONUNG_SOURCE_OF_TRUTH.md` §14) ---

/// Parse the `Settlement[20]` table and its per-record waypoint lists. The
/// waypoint list ends at the first all-zero 8-byte entry.
///
/// Citizens carry a per-location **home** waypoint (owner == their Entity
/// index) and, when `archetype & 0x38 != 0`, a role **station** waypoint.
pub fn parse_settlements(data: &[u8]) -> Result<Vec<Settlement>, String> {
    let required = SETTLEMENT_OFF + SETTLEMENT_COUNT * SETTLEMENT_STRIDE;
    if data.len() < required {
        return Err(format!(
            "Settlement table truncated: file is {} bytes, need at least {} (0x{:X})",
            data.len(),
            required,
            required
        ));
    }
    let mut out = Vec::with_capacity(SETTLEMENT_COUNT);
    for index in 0..SETTLEMENT_COUNT {
        let o = SETTLEMENT_OFF + index * SETTLEMENT_STRIDE;
        out.push(parse_settlement_record(
            index,
            &data[o..o + SETTLEMENT_STRIDE],
        ));
    }
    Ok(out)
}

/// Parse one 0x4B5-byte `Settlement` record (same layout in `Game.N` and the
/// legacy runtime snapshot).
pub fn parse_settlement_record(index: usize, data: &[u8]) -> Settlement {
    let loc_id = data[3];
    let group_id = data[2];
    let flags_4b2 = data[0x4B2];
    // `FUN_00420F30`: target count = u8[+0] + u8[+1] + 7; entries at +0x18.
    let target_count = data[0] as usize + data[1] as usize + 7;
    let mut targets = Vec::with_capacity(target_count.min(120));
    for t in 0..target_count {
        let to = 0x18 + t * 8;
        if to + 8 > data.len() {
            break;
        }
        targets.push(SettlementTarget {
            key: data[to],
            state: data[to + 1],
            prop_index: u16(data, to + 2) as u16,
            timer: u16(data, to + 6) as u16,
        });
    }
    let mut waypoints = Vec::new();
    for w in 0..WAYPOINT_MAX {
        let wo = WAYPOINT_OFF + w * WAYPOINT_STRIDE;
        if wo + WAYPOINT_STRIDE > data.len() {
            break;
        }
        let facing = data[wo];
        let dialogue_id = data[wo + 1];
        let owner = u16(data, wo + 2) as u16;
        let x = u16(data, wo + 4) as u16;
        let y = u16(data, wo + 6) as u16;
        if facing == 0 && dialogue_id == 0 && owner == 0 && x == 0 && y == 0 {
            break;
        }
        waypoints.push(Waypoint {
            facing,
            dialogue_id,
            owner,
            x,
            y,
        });
    }
    Settlement {
        index,
        loc_id,
        group_id,
        progress_14: u16(data, 0x14) as u16,
        money_pool_10: i32v(data, 0x10),
        target_base0: data[0],
        target_base1: data[1],
        apprentice_start: [i32v(data, 0x3FC), i32v(data, 0x400), i32v(data, 0x404)],
        flags_4b2,
        targets,
        waypoints,
    }
}

/// Parse the settlement merchant stock lists and restock state
/// (`FUN_00431E2C` / `FUN_004175FC` / `FUN_004179C8`). The same `Game.N`
/// `Settlement` records hold these fields, but they are kept in a parallel
/// table so non-trade settlement consumers are unaffected.
pub fn parse_settlement_trade(data: &[u8]) -> Result<Vec<SettlementTrade>, String> {
    let required = SETTLEMENT_OFF + SETTLEMENT_COUNT * SETTLEMENT_STRIDE;
    if data.len() < required {
        return Err(format!(
            "Settlement table truncated: file is {} bytes, need at least {} (0x{:X})",
            data.len(),
            required,
            required
        ));
    }
    let mut out = Vec::with_capacity(SETTLEMENT_COUNT);
    for index in 0..SETTLEMENT_COUNT {
        let o = SETTLEMENT_OFF + index * SETTLEMENT_STRIDE;
        out.push(parse_settlement_trade_record(
            index,
            &data[o..o + SETTLEMENT_STRIDE],
        ));
    }
    Ok(out)
}

/// Parse one 0x4B5-byte settlement record's trade fields (same layout in
/// `Game.N` and the legacy runtime snapshot).
pub fn parse_settlement_trade_record(index: usize, data: &[u8]) -> SettlementTrade {
    let mut stock2 = [0u16; TRADE_STOCK2_COUNT];
    for (i, slot) in stock2.iter_mut().enumerate() {
        *slot = u16(data, TRADE_STOCK2_OFF + i * 2) as u16;
    }
    let mut stock3 = [0u16; TRADE_STOCK3_COUNT];
    for (i, slot) in stock3.iter_mut().enumerate() {
        *slot = u16(data, TRADE_STOCK3_OFF + i * 2) as u16;
    }
    let mut stock4 = [0u16; TRADE_STOCK4_COUNT];
    for (i, slot) in stock4.iter_mut().enumerate() {
        *slot = u16(data, TRADE_STOCK4_OFF + i * 2) as u16;
    }
    SettlementTrade {
        index,
        loc_id: data[3],
        flags_4b2: data[0x4B2],
        stock2,
        stock3,
        stock4,
        prod_role2: u16(data, TRADE_PROD2_OFF) as u16,
        prod_role3: u16(data, TRADE_PROD3_OFF) as u16,
        prod_role4: u16(data, TRADE_PROD4_OFF) as u16,
        prod_role5: u16(data, TRADE_PROD5_OFF) as u16,
        timer2: i32v(data, TRADE_TIMER2_OFF),
        timer3: i32v(data, TRADE_TIMER3_OFF),
        timer4: i32v(data, TRADE_TIMER4_OFF),
        last_slot: data[TRADE_LAST_SLOT_OFF] as i8,
        quota0: u16(data, TRADE_QUOTA0_OFF) as u16,
        quota1: u16(data, TRADE_QUOTA1_OFF) as u16,
        quota2: u16(data, TRADE_QUOTA2_OFF) as u16,
    }
}
