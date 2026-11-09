//! Executable quest/dialogue opcode handlers (`FUN_0043159C`).
//!
//! The original budget/action programs run against live game globals. The
//! remake evaluates one page against an owned [`QuestContext`] snapshot so that
//! `B4` predicates may also perform the `Both`-kind side effects, then applies
//! the snapshot back to [`PlayableApp`] once. This keeps the `world.res.quest`
//! borrow independent of the mutable game state.
//!
//! Opcode semantics are recovered from `konung.res` (see `QUEST_OPCODES.md`).
//! `0x16A` is intentionally a documented no-op: its float evaluation body was
//! not fully recoverable.

use super::*;

use crate::actors::PartyMember;
use crate::combat::OriginalRng;
use crate::constants::exe_tables::SETTLEMENT_TARGET_TABLE as SETTLEMENT_REQUIREMENT;
use crate::population::{GameTables, Group, Settlement, SettlementTarget, SettlementTrade};
use crate::quest::QuestData;
use crate::save::SaveExtra;
use crate::script::{action_steps, low_family, ActionStep, LowFamily};
use crate::world_objects::ActiveWorldObject;

/// Item class used by the parameterized low-opcode families.
const PARTY_ITEM_CLASS: u8 = 0x0B;
/// Inventory slots on an Entity.
const INVENTORY_SLOTS: usize = 32;
/// Item-instance record size.
const ITEM_INSTANCE_SIZE: usize = 16;
/// Number of item-instance slots in the loaded `Game.N` table.
const ITEM_INSTANCE_COUNT: usize = 8192;

/// Owned mutable snapshot of the state a dialog program can read or change.
pub(super) struct QuestContext {
    pub player: PlayerCharacter,
    pub player_x: f64,
    pub player_y: f64,
    pub quest_flags: Vec<u32>,
    pub settlements: Vec<Settlement>,
    pub settlement_trade: Vec<SettlementTrade>,
    pub tables: Option<GameTables>,
    pub party: Vec<PartyMember>,
    pub party_selected: [bool; crate::hud::PARTY_CELLS],
    pub active_world_objects: Vec<ActiveWorldObject>,
    pub item_table: Vec<u8>,
    pub rng: OriginalRng,
    pub npc_entity: usize,
    pub current_map: u32,
    pub world_frame: u64,
    pub trade_request: Option<usize>,
    pub unknown_ops: Vec<u16>,
}

impl QuestContext {
    /// Build the snapshot for a dialogue with `npc_entity`.
    pub fn from_app(app: &PlayableApp, npc_entity: usize) -> QuestContext {
        QuestContext {
            player: app.player,
            player_x: app.player_x,
            player_y: app.player_y,
            quest_flags: app.quest_flags.clone(),
            settlements: app.settlements.clone(),
            settlement_trade: app.settlement_trade.clone(),
            tables: app.tables.clone(),
            party: app.party.clone(),
            party_selected: app.party_selected,
            active_world_objects: app.active_world_objects.clone(),
            item_table: app.world.res.save_game.clone(),
            rng: app.combat_rng.clone(),
            npc_entity,
            current_map: app.current_map,
            world_frame: app.world_frame,
            trade_request: None,
            unknown_ops: Vec::new(),
        }
    }

    /// Apply the snapshot back to the live game. Population is invalidated when
    /// actor tables, settlements or the party changed.
    pub fn apply_to_app(self, app: &mut PlayableApp) {
        let tables_changed = app.tables.as_ref() != self.tables.as_ref();
        let party_changed = app.party != self.party;
        let settlements_changed = app.settlements != self.settlements;
        let trade = self.trade_request;

        app.player = self.player;
        app.player_x = self.player_x;
        app.player_y = self.player_y;
        app.tables = self.tables;
        app.party = self.party;
        app.party_selected = self.party_selected;
        app.settlements = self.settlements;
        app.settlement_trade = self.settlement_trade;
        app.quest_flags = self.quest_flags;
        app.combat_rng = self.rng;
        app.active_world_objects = self.active_world_objects;
        app.world.res.set_save_game(&self.item_table);
        if tables_changed || party_changed || settlements_changed {
            app.population_cache.remove(&app.current_map);
        }
        if !app.party_selected.iter().any(|&v| v) {
            app.party_selected[0] = true;
        }
        if let Some(npc) = trade {
            app.start_trade(npc);
        }
    }

    // --- B7 flags ---------------------------------------------------------

    fn set_flag(&mut self, index: usize) {
        if let Some(w) = self.quest_flags.get_mut(index) {
            *w |= 0x80;
        }
    }

    fn clear_flag(&mut self, index: usize) {
        if let Some(w) = self.quest_flags.get_mut(index) {
            *w &= !0x80;
        }
    }

    // --- Settlement helpers ----------------------------------------------

    fn settlement_index(&self) -> Option<usize> {
        self.settlements
            .iter()
            .position(|s| s.loc_id as u32 == self.current_map)
    }

    fn settlement(&self) -> Option<&Settlement> {
        self.settlement_index().and_then(|i| self.settlements.get(i))
    }

    fn settlement_mut(&mut self) -> Option<&mut Settlement> {
        let i = self.settlement_index()?;
        self.settlements.get_mut(i)
    }

    fn target(&self, index: usize) -> Option<&SettlementTarget> {
        self.settlement()?.targets.get(index)
    }

    fn target_state_is_3(&self, index: usize) -> bool {
        self.target(index).map(|t| t.state == 3).unwrap_or(false)
    }

    fn target_base(&self) -> Option<(u8, u8)> {
        self.settlement().map(|s| (s.target_base0, s.target_base1))
    }

    fn requirement(index: usize) -> (u16, u16) {
        *SETTLEMENT_REQUIREMENT.get(index).unwrap_or(&(0, 0))
    }

    fn target_key(&self, index: usize) -> Option<usize> {
        self.target(index).map(|t| t.key as usize)
    }

    // --- Actors / groups --------------------------------------------------

    fn entity(&self, id: usize) -> Option<&crate::population::Entity> {
        self.tables.as_ref()?.entities.get(id)
    }

    fn entity_or_party(&self, id: usize) -> Option<&crate::population::Entity> {
        self.party
            .iter()
            .find(|m| m.entity.index == id)
            .map(|m| &m.entity)
            .or_else(|| self.entity(id))
    }

    fn entity_or_party_mut(&mut self, id: usize) -> Option<&mut crate::population::Entity> {
        if let Some(pos) = self.party.iter().position(|m| m.entity.index == id) {
            return Some(&mut self.party[pos].entity);
        }
        self.tables.as_mut()?.entities.get_mut(id)
    }

    fn npc(&self) -> Option<&crate::population::Entity> {
        self.entity_or_party(self.npc_entity)
    }

    fn npc_mut(&mut self) -> Option<&mut crate::population::Entity> {
        let id = self.npc_entity;
        self.entity_or_party_mut(id)
    }

    fn npc_in_party(&self) -> bool {
        self.party.iter().any(|m| m.entity.index == self.npc_entity)
    }

    fn group_record_by_index(&self, index: usize) -> Option<&Group> {
        self.tables.as_ref()?.groups.get(index)
    }

    fn object_group_for_location(&self, loc: u16) -> Option<&Group> {
        self.tables
            .as_ref()?
            .groups
            .iter()
            .find(|g| g.loc_id as u16 == loc && g.class_1e & 0x40 != 0)
    }

    fn group_entity_ids_by_index(&self, index: usize) -> Vec<usize> {
        let Some(g) = self.group_record_by_index(index) else {
            return Vec::new();
        };
        let Ok(range) = g.entity_range() else {
            return Vec::new();
        };
        range.collect()
    }

    /// Reserved party group: the protagonist plus the recruited companions.
    /// The original `FUN_004313B0` scans group 0 starting at member 1 (the
    /// leader is skipped), so the remake's companion-only list is equivalent.
    fn party_ids(&self) -> Vec<usize> {
        self.party.iter().map(|m| m.entity.index).collect()
    }

    /// `0x00..=0x37` (`FUN_00432F8C`): the operand is a **location id**. Find
    /// the first object group (`class & 0x40`) in that location; the group is
    /// active when its target count exceeds its active count or when any
    /// member carries the `0x80` marker.
    fn group_active(&mut self, location: u16) -> bool {
        let Some(g) = self.object_group_for_location(location) else {
            return false;
        };
        if (g.entity_count as i8) < (g.count_18 as i8) {
            return true;
        }
        let Ok(range) = g.entity_range() else {
            return false;
        };
        self.tables
            .as_ref()
            .map(|t| {
                t.entities[range]
                    .iter()
                    .any(|e| e.archetype & 0x80 != 0)
            })
            .unwrap_or(false)
    }

    /// `0x38..=0x6F` (`FUN_00433FF8`): the operand is a location id. True when
    /// the location has no object group or the first object group has no
    /// visible (non-corpse) member.
    fn group_has_no_live_member(&mut self, location: u16) -> bool {
        let Some(g) = self.object_group_for_location(location) else {
            return true;
        };
        let Ok(range) = g.entity_range() else {
            return true;
        };
        self.tables
            .as_ref()
            .map(|t| {
                !t.entities[range].iter().any(|e| {
                    e.archetype & 0x80 == 0 && e.kind != 9 && e.kind != 3
                })
            })
            .unwrap_or(true)
    }

    fn settlement_group_building(&mut self) -> u16 {
        let Some(group_index) = self.settlement().map(|s| s.group_id as usize) else {
            return 0;
        };
        let ids = self.group_entity_ids_by_index(group_index);
        ids.into_iter()
            .filter_map(|id| self.entity_or_party(id).map(|e| e.stats.building))
            .max()
            .unwrap_or(0)
    }

    fn skill_check(&mut self, param: u8) -> bool {
        let ids = self.party_ids();
        // `FUN_004313B0` scans party group 0 from member 1; the protagonist is
        // held separately in the remake, so the companion list is the scan.
        for id in ids {
            let Some(e) = self.entity_or_party(id) else {
                continue;
            };
            if e.archetype & 0x80 != 0 {
                continue;
            }
            if (e.archetype & 0x38) == param.wrapping_shl(3) {
                return true;
            }
            match param {
                1 => {
                    if e.stats.healing > 100 {
                        return true;
                    }
                }
                2 => {
                    if e.stats.trade > 0x1e {
                        return true;
                    }
                }
                3 => {
                    if e.stats.smithing > 4 {
                        return true;
                    }
                }
                4 => {
                    if e.stats.strength_base > 10 && e.stats.melee_mastery > 10 {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

    fn find_skill_member(&mut self, slot: u8) -> Option<usize> {
        let ids = self.party_ids();
        for &id in &ids {
            let Some(e) = self.entity_or_party(id) else {
                continue;
            };
            if e.archetype & 0x80 != 0 {
                continue;
            }
            if (e.archetype & 0x38) == slot.wrapping_shl(3) {
                return Some(id);
            }
            let matches = match slot {
                1 => e.stats.healing > 100,
                2 => e.stats.trade > 0x1e,
                3 => e.stats.smithing > 4,
                4 => e.stats.strength_base > 10 && e.stats.melee_mastery > 10,
                _ => false,
            };
            if matches {
                return Some(id);
            }
        }
        None
    }

    // --- Items ------------------------------------------------------------

    fn item_instance(&self, id: u16) -> Option<(u8, u8, u16)> {
        if id == 0 {
            return None;
        }
        let o = id as usize * ITEM_INSTANCE_SIZE;
        if o + ITEM_INSTANCE_SIZE > self.item_table.len() {
            return None;
        }
        let class = self.item_table[o];
        if (class as i8) < 0 {
            return None;
        }
        let subtype = self.item_table[o + 3];
        let flags = u16::from_le_bytes([self.item_table[o + 14], self.item_table[o + 15]]);
        Some((class, subtype, flags))
    }

    fn inventory_contains(&self, class: u8, subtype: u8) -> bool {
        self.player
            .inventory
            .iter()
            .any(|&id| self.item_instance(id).map(|(c, s, _)| (c, s) == (class, subtype)).unwrap_or(false))
    }

    fn inventory_slot(&self, class: u8, subtype: u8) -> Option<usize> {
        self.player.inventory.iter().position(|&id| {
            self.item_instance(id)
                .map(|(c, s, _)| (c, s) == (class, subtype))
                .unwrap_or(false)
        })
    }

    fn alloc_item(&mut self, class: u8, subtype: u8) -> Option<u16> {
        if self.item_table.len() < ITEM_INSTANCE_COUNT * ITEM_INSTANCE_SIZE {
            return None;
        }
        for id in 1..ITEM_INSTANCE_COUNT {
            let o = id * ITEM_INSTANCE_SIZE;
            if (self.item_table[o] as i8) < 0 {
                self.item_table[o..o + ITEM_INSTANCE_SIZE].fill(0);
                self.item_table[o] = class;
                self.item_table[o + 3] = subtype;
                return Some(id as u16);
            }
        }
        None
    }

    fn add_inventory_item(&mut self, class: u8, subtype: u8) -> bool {
        // The original checks for a free slot before allocating the instance.
        let Some(slot) = crate::inventory::free_inventory_slot(&self.player.inventory) else {
            return false;
        };
        let Some(id) = self.alloc_item(class, subtype) else {
            return false;
        };
        self.player.inventory[slot] = id;
        true
    }

    fn remove_inventory_item(&mut self, class: u8, subtype: u8) -> bool {
        let Some(slot) = self.inventory_slot(class, subtype) else {
            return false;
        };
        let id = self.player.inventory[slot];
        if id != 0 {
            let o = id as usize * ITEM_INSTANCE_SIZE;
            if o < self.item_table.len() {
                self.item_table[o] = 0xFF;
            }
        }
        for i in slot..INVENTORY_SLOTS - 1 {
            self.player.inventory[i] = self.player.inventory[i + 1];
        }
        self.player.inventory[INVENTORY_SLOTS - 1] = 0;
        true
    }

    fn drop_item_at_player(&mut self, id: u16) {
        if id == 0 {
            return;
        }
        self.active_world_objects.push(ActiveWorldObject {
            source: usize::MAX,
            x: self.player_x.round() as i32,
            y: self.player_y.round() as i32,
            money: 0,
            items: vec![id],
            sprite: crate::world_objects::WORLD_OBJECT_GENERIC_SPRITE,
        });
    }

    fn equipment_has_flag(&self, bit: u16) -> bool {
        self.player
            .equipment
            .iter()
            .chain(std::iter::once(&self.player.special_equipment))
            .any(|&id| {
                self.item_instance(id)
                    .map(|(_, _, flags)| flags & bit != 0)
                    .unwrap_or(false)
            })
    }

    /// `0x16A` (`FUN_00434880`): repair/cap the party's gear using the dialogue
    /// NPC's smithing skill. For every live party member's six equipment slots
    /// and every inventory item of signed class `<= 5`:
    ///
    /// - threshold = `npc.smithing * 10`
    /// - if `max_durability > threshold`, the excess is reduced by 30%
    /// - `current_durability = max_durability`
    ///
    /// The original's follow-up `target.smithing` update is dead code: it
    /// computes `(max - current)` after setting `current = max`, so the value is
    /// always zero and the update is skipped.
    fn repair_party_gear(&mut self) -> bool {
        let smithing = self
            .npc()
            .map(|e| e.stats.smithing)
            .unwrap_or(0);
        let threshold = smithing as f64 * 10.0;

        let equipment = self.player.equipment;
        let inventory = self.player.inventory;
        for id in equipment {
            self.cap_item_durability(id, threshold, false);
        }
        for id in inventory {
            self.cap_item_durability(id, threshold, true);
        }

        let party_len = self.party.len();
        for i in 0..party_len {
            let entity = &self.party[i].entity;
            if entity.archetype & 0x80 != 0 || entity.kind == 9 || entity.kind == 3 {
                continue;
            }
            let equipment = entity.equipment;
            let inventory = entity.inventory;
            for id in equipment {
                self.cap_item_durability(id, threshold, false);
            }
            for id in inventory {
                self.cap_item_durability(id, threshold, true);
            }
        }
        true
    }

    /// Apply the `0x16A` durability cap/repair to one item instance.
    /// `inventory_filter` mirrors the original signed `class <= 5` gate for
    /// inventory items (equipment slots are unconditional).
    fn cap_item_durability(&mut self, id: u16, threshold: f64, inventory_filter: bool) -> bool {
        if id == 0 {
            return false;
        }
        let o = id as usize * ITEM_INSTANCE_SIZE;
        if o + 12 > self.item_table.len() {
            return false;
        }
        if inventory_filter {
            let class = self.item_table[o] as i8;
            if class > 5 {
                return false;
            }
        }
        let max = f32::from_le_bytes([
            self.item_table[o + 8],
            self.item_table[o + 9],
            self.item_table[o + 10],
            self.item_table[o + 11],
        ]);
        if !max.is_finite() {
            return false;
        }
        let threshold_f32 = threshold as f32;
        let new_max = if max > threshold_f32 {
            (threshold + (max - threshold_f32) as f64 * 0.7) as f32
        } else {
            max
        };
        self.item_table[o + 8..o + 12].copy_from_slice(&new_max.to_le_bytes());
        self.item_table[o + 4..o + 8].copy_from_slice(&new_max.to_le_bytes());
        true
    }

    fn recalc_identification(&mut self) {
        let mut ids: Vec<u16> = Vec::new();
        ids.extend_from_slice(&self.player.equipment);
        ids.push(self.player.special_equipment);
        ids.extend_from_slice(&self.player.inventory);
        ids.extend_from_slice(&self.player.additional_equipment);
        for id in ids {
            if id == 0 {
                continue;
            }
            let o = id as usize * ITEM_INSTANCE_SIZE;
            if o + ITEM_INSTANCE_SIZE > self.item_table.len() {
                continue;
            }
            let flags = u16::from_le_bytes([self.item_table[o + 14], self.item_table[o + 15]]);
            if flags & 1 != 0 {
                self.player.identification = self.player.identification.saturating_add(1);
                let next = flags ^ 1;
                self.item_table[o + 14..o + 16].copy_from_slice(&next.to_le_bytes());
            }
        }
        self.player.identification = self.player.identification.min(100);
    }

    // --- Party / world actors --------------------------------------------

    fn recruit_entity(&mut self, id: usize) -> bool {
        if self.party.iter().any(|m| m.entity.index == id) {
            return true;
        }
        let Some(mut entity) = self.entity_or_party(id).cloned() else {
            return false;
        };
        // `FUN_00431820` applies the apprentice's trained profession skill
        // before inserting the NPC into the party. `gain = (elapsed/16)/0x5A0`
        // and the skill is blended toward the producer's value with the
        // `0x4505C8` factor 0.03 (`FUN_0043eaae` is a plain round helper).
        let role = ((entity.archetype & 0x38) >> 3) as usize;
        if (1..=3).contains(&role) {
            if let Some(si) = self.settlement_index() {
                let start = self
                    .settlements
                    .get(si)
                    .map(|s| s.apprentice_start[role - 1])
                    .unwrap_or(0);
                let gain = ((self.world_frame as i64 - start as i64).max(0) / 16 / 0x5A0) as i32;
                // The producer at `waypoint[role]` is the skill cap/master.
                let producer = self
                    .settlements
                    .get(si)
                    .map(|s| s.producer(role) as usize)
                    .unwrap_or(0);
                let master_skill = self
                    .tables
                    .as_ref()
                    .and_then(|t| t.entities.get(producer))
                    .map(|e| match role {
                        1 => e.stats.healing,
                        2 => e.stats.trade,
                        _ => e.stats.smithing,
                    })
                    .unwrap_or(0);
                match role {
                    // Role 1: healing `+0x48`, clamp 500.
                    1 => {
                        let own = entity.stats.healing as f64;
                        let value = if producer == 0 {
                            own + gain as f64 * 0.03
                        } else {
                            own + (master_skill as f64 - own) * 0.03 * gain as f64
                        };
                        entity.stats.healing = value.round_ties_even().clamp(0.0, 500.0) as u16;
                    }
                    // Role 2: trade `+0x42`, capped by the merchant and 100.
                    2 => {
                        let mut trade = entity
                            .stats
                            .trade
                            .saturating_add(gain.max(0) as u16);
                        if producer != 0 && master_skill < trade {
                            trade = master_skill;
                        }
                        entity.stats.trade = trade.min(100);
                    }
                    // Role 3: smithing `+0x4C`, no clamp in the original.
                    _ => {
                        let own = entity.stats.smithing as f64;
                        let value = if producer == 0 {
                            own + gain as f64 * 0.03
                        } else {
                            own + (master_skill as f64 - own) * 0.03 * gain as f64
                        };
                        entity.stats.smithing = value
                            .round_ties_even()
                            .clamp(0.0, u16::MAX as f64) as u16;
                    }
                }
            }
        }
        // `FUN_00431820` moves the entity into reserved group 0 (`+0x05 = 0`).
        entity.group_id = 0;
        let cell = (entity.x as i32, entity.y as i32);
        let mut motion = crate::actors::ActorMotion::default();
        motion.cell = cell;
        motion.render_cell = cell;
        motion.facing = entity.facing & 7;
        motion.action = crate::anim::idle_action(crate::anim::battle_from_flags(entity.flags_b)) as u8;
        self.party.push(PartyMember { entity, motion });
        let cell_index = self.party.len();
        if cell_index < crate::hud::PARTY_CELLS {
            self.party_selected[cell_index] = true;
        }
        true
    }

    fn dismiss_entity(&mut self, id: usize) -> bool {
        let Some(index) = self.party.iter().position(|m| m.entity.index == id) else {
            return false;
        };
        self.party.remove(index);
        let cell = index + 1;
        for j in cell..crate::hud::PARTY_CELLS - 1 {
            self.party_selected[j] = self.party_selected[j + 1];
        }
        self.party_selected[crate::hud::PARTY_CELLS - 1] = false;
        true
    }

    fn remove_world_entity(&mut self, id: usize) -> bool {
        self.dismiss_entity(id);
        self.detach_table_entity(id)
    }

    /// Remove `id` from its Group range, compacting later entities and
    /// adjusting party indices/waypoint owners. Does not touch the party list.
    fn detach_table_entity(&mut self, id: usize) -> bool {
        let mut removed = false;
        let mut shifted_range = None;
        if let Some(tables) = self.tables.as_mut() {
            let mut group_index = None;
            for g in &tables.groups {
                if let Ok(range) = g.entity_range() {
                    if range.contains(&id) {
                        group_index = Some(g.index);
                        break;
                    }
                }
            }
            if let Some(gi) = group_index {
                if let Some(g) = tables.groups.get(gi) {
                    let Ok(range) = g.entity_range() else {
                        return false;
                    };
                    let end = range.end;
                    if id + 1 < end {
                        for i in id..end - 1 {
                            tables.entities[i] = tables.entities[i + 1].clone();
                            tables.entities[i].index = i;
                        }
                    }
                    if let Some(g) = tables.groups.get_mut(gi) {
                        g.entity_count = g.entity_count.saturating_sub(1);
                        g.count_18 = g.count_18.saturating_sub(1);
                    }
                    shifted_range = Some((id, end));
                    removed = true;
                }
            }
        }
        if let Some((start, end)) = shifted_range {
            for m in &mut self.party {
                if m.entity.index > start && m.entity.index < end {
                    m.entity.index -= 1;
                }
            }
        }
        if let Some(settlement) = self.settlement_mut() {
            for w in settlement.waypoints.iter_mut().take(5) {
                if w.owner as usize == id {
                    w.owner = 0;
                } else if w.owner as usize > id {
                    w.owner = w.owner.saturating_sub(1);
                }
            }
        }
        removed
    }

    /// `FUN_00432954`: move an NPC (world actor or party member) into the
    /// settlement group's next free slot. The optional waypoint slot (0..=4)
    /// is assigned when it is unowned, like the original home/role station.
    fn settle_into_settlement(&mut self, id: usize, waypoint_slot: Option<u8>) -> bool {
        let Some(si) = self.settlement_index() else {
            return false;
        };
        let Some(settlement) = self.settlements.get(si).cloned() else {
            return false;
        };
        let group_index = settlement.group_id as usize;
        let Some(group) = self.group_record_by_index(group_index).cloned() else {
            return false;
        };
        if group.entity_count >= group.capacity {
            return false;
        }
        let Some(mut entity) = self
            .party
            .iter()
            .find(|m| m.entity.index == id)
            .map(|m| m.entity.clone())
            .or_else(|| self.entity(id).cloned())
        else {
            return false;
        };
        let was_party = self.party.iter().any(|m| m.entity.index == id);
        if was_party {
            self.dismiss_entity(id);
        }
        // Remove the source from its old table group. The destination slot is
        // recomputed afterwards because the removal may shift indices.
        self.detach_table_entity(id);
        let Some(group) = self.group_record_by_index(group_index).cloned() else {
            return false;
        };
        let dest = group.first_entity as usize + group.entity_count as usize;
        let Some(tables) = self.tables.as_mut() else {
            return false;
        };
        if dest >= tables.entities.len() {
            return false;
        }
        let waypoint = waypoint_slot
            .and_then(|slot| self.settlements.get(si)?.waypoints.get(slot as usize).copied())
            .filter(|w| w.owner == 0);
        entity.index = dest;
        entity.group_id = settlement.group_id;
        entity.flags_a = 0x60;
        if let Some(w) = waypoint {
            entity.x = w.x;
            entity.y = w.y;
            entity.facing = w.facing;
        }
        tables.entities[dest] = entity;
        if let Some(g) = tables.groups.get_mut(group_index) {
            g.entity_count = g.entity_count.saturating_add(1);
            g.count_18 = g.count_18.saturating_add(1);
        }
        if let Some(s) = self.settlements.get_mut(si) {
            if let Some(slot) = waypoint_slot {
                if let Some(w) = s.waypoints.get_mut(slot as usize) {
                    if w.owner == 0 {
                        w.owner = dest as u16;
                    }
                }
            }
        }
        true
    }

    /// True when the entity owns one of the settlement's five home waypoints.
    fn owns_home_waypoint(&self, id: usize) -> bool {
        self.settlement()
            .map(|s| {
                s.waypoints
                    .iter()
                    .take(5)
                    .any(|w| w.owner as usize == id && w.owner != 0)
            })
            .unwrap_or(false)
    }

    /// The producer role (`0x08`/`0x10`/`0x18`) and timer index (`0..3`) of the
    /// dialogue NPC, or `None` when it is not settlement producer 2/3/4.
    fn dialog_producer_role(&self) -> Option<(u8, usize)> {
        let si = self.settlement_index()?;
        let s = self.settlements.get(si)?;
        let id = self.npc_entity as u16;
        if s.producer(1) != 0 && s.producer(1) == id {
            Some((0x08, 0))
        } else if s.producer(2) != 0 && s.producer(2) == id {
            Some((0x10, 1))
        } else if s.producer(3) != 0 && s.producer(3) == id {
            Some((0x18, 2))
        } else {
            None
        }
    }

    /// `FUN_00432804` / `FUN_00434364` candidate scan: the first settlement
    /// member with one of the human visuals, no profession role, visible,
    /// not a corpse, and no owned home waypoint.
    fn apprentice_candidate(&self) -> Option<usize> {
        let si = self.settlement_index()?;
        let s = self.settlements.get(si)?;
        for id in self.group_entity_ids_by_index(s.group_id as usize) {
            let Some(e) = self.entity(id) else {
                continue;
            };
            if e.visual_index >= 3
                || e.archetype & 0x38 != 0
                || e.archetype & 0x80 != 0
                || e.kind == 9
                || e.kind == 3
                || self.owns_home_waypoint(id)
            {
                continue;
            }
            return Some(id);
        }
        None
    }

    /// `0x162`: mark the eligible villager as the apprentice for the dialogue
    /// NPC's producer role. This does **not** recruit into the party; the
    /// actual join is `0x123` on the apprentice's later dialogue.
    fn prepare_apprentice(&mut self) -> bool {
        let Some((role, timer_index)) = self.dialog_producer_role() else {
            return false;
        };
        let Some(id) = self.apprentice_candidate() else {
            return false;
        };
        let Some(table) = self.tables.as_mut() else {
            return false;
        };
        let Some(e) = table.entities.get_mut(id) else {
            return false;
        };
        e.archetype |= role;
        // `FUN_00414A34` clears flags then re-applies the active bit; the
        // authored citizens already carry `0x40`.
        e.flags_a |= 0x40;
        let frame = self.world_frame as i32;
        if let Some(s) = self.settlement_mut() {
            s.apprentice_start[timer_index] = frame;
        }
        true
    }

    fn prepare_ally(&mut self, slot: u8) -> bool {
        let Some(settlement) = self.settlement() else {
            return false;
        };
        if settlement
            .waypoints
            .get(slot as usize)
            .map(|w| w.owner)
            .unwrap_or(0)
            != 0
        {
            return false;
        }
        let Some(id) = self.find_skill_member(slot) else {
            return false;
        };
        // `FUN_00431654`: settle the party specialist into the village and
        // station them at waypoint `slot` (1..=4), not recruit them.
        self.settle_into_settlement(id, Some(slot))
    }

    /// `0x14E` heals the protagonist at the NPC healer's expense of skill
    /// (`FUN_00433B18`): the **NPC's** healing skill rises by the missing-HP
    /// term and the leader's HP is restored. `0x14F` does the same for the
    /// whole party (`FUN_00433BA0`).
    fn heal_target(&mut self, group_wide: bool) -> bool {
        let base = self
            .npc()
            .map(|e| e.stats.healing as f64)
            .unwrap_or(0.0);
        if group_wide {
            let player_hp = self.player.hp_internal as i32;
            let delta = (640 - player_hp) as f64 / 1024.0 + base;
            self.player.healing = delta.round().clamp(0.0, u16::MAX as f64) as u16;
            self.player.hp_internal = 1600;
            for m in self.party.iter_mut() {
                if m.motion.dead {
                    continue;
                }
                let delta = (640 - m.entity.stats.hp_internal as i32) as f64 / 1024.0 + base;
                m.entity.stats.healing =
                    delta.round().clamp(0.0, u16::MAX as f64) as u16;
                m.entity.stats.hp_internal = 1600;
            }
            true
        } else {
            let player_hp = self.player.hp_internal as i32;
            let delta = (1600 - player_hp) as f64 / 1024.0 + base;
            if let Some(e) = self.npc_mut() {
                e.stats.healing = delta.round().clamp(0.0, u16::MAX as f64) as u16;
            }
            self.player.hp_internal = 1600;
            true
        }
    }

    // --- Programs ---------------------------------------------------------

    pub fn run_action_program(&mut self, quest: &QuestData, id: u16) {
        let words = quest.action_program(id);
        for step in action_steps(&words) {
            match step {
                ActionStep::Op(op) => {
                    self.exec_op(op);
                }
                ActionStep::SetFlag(i) => self.set_flag(i),
                ActionStep::ClearFlag(i) => self.clear_flag(i),
            }
        }
    }

    /// Execute one script opcode. Returns the original handler's boolean result.
    pub fn exec_op(&mut self, op: u16) -> bool {
        if let Some(low) = low_family(op) {
            return match low {
                LowFamily::GroupActive { location } => self.group_active(location),
                LowFamily::GroupNoLiveMember { location } => {
                    self.group_has_no_live_member(location)
                }
                LowFamily::RemovePartyItem { subtype } => {
                    self.remove_inventory_item(PARTY_ITEM_CLASS, subtype)
                }
                LowFamily::SpawnPartyItem { subtype } => {
                    let Some(id) = self.alloc_item(PARTY_ITEM_CLASS, subtype) else {
                        return false;
                    };
                    if crate::inventory::insert_inventory_front(&mut self.player.inventory, id) {
                        true
                    } else {
                        self.drop_item_at_player(id);
                        true
                    }
                }
                LowFamily::HasPartyItem { subtype } => {
                    self.inventory_contains(PARTY_ITEM_CLASS, subtype)
                }
            };
        }

        match op {
            0x112 => {
                let group = self.npc().map(|e| e.group_id).unwrap_or(0);
                if let Some(tables) = self.tables.as_mut() {
                    if let Some(g) = tables.groups.get_mut(group as usize) {
                        // `FUN_00431DCC`: `Group+0x1D = 1` (activate this NPC's
                        // group), not a separate "visited" flag.
                        g.activity_1d = 1;
                    }
                }
                let settlement_group = self.settlement().map(|s| s.group_id).unwrap_or(0xFF);
                if settlement_group == group {
                    if let Some(s) = self.settlement_mut() {
                        s.flags_4b2 |= 0x15;
                    }
                }
                true
            }
            0x113 => {
                self.trade_request = Some(self.npc_entity);
                true
            }
            0x114 => {
                self.player.state_d3 = 0x46;
                true
            }
            0x115 => self.rng.next_u32() % 100 < 50,
            0x116 => self.skill_check(4),
            0x117 => self.skill_check(1),
            0x118 => self.skill_check(3),
            0x119 => self.skill_check(2),
            0x11A => {
                if self.settlement().is_none() || self.player.money < 120 {
                    return false;
                }
                self.player.money -= 120;
                if let Some(s) = self.settlement_mut() {
                    s.flags_4b2 &= !0x20;
                }
                true
            }
            0x11B => {
                let Some(pool) = self.settlement().map(|s| s.money_pool_10) else {
                    return false;
                };
                self.player.money += pool;
                if let Some(s) = self.settlement_mut() {
                    s.money_pool_10 = 0;
                }
                true
            }
            0x11C => {
                if self.settlement().is_none() {
                    return false;
                }
                if let Some(s) = self.settlement_mut() {
                    s.flags_4b2 |= 0x20;
                }
                self.player.money += 100;
                false
            }
            0x11D => {
                self.player.money -= 5;
                true
            }
            0x11E => {
                self.player.money -= 10;
                true
            }
            0x11F => {
                self.player.money -= 20;
                true
            }
            0x120 => {
                self.player.money -= 50;
                true
            }
            0x121 => {
                self.player.money -= 100;
                true
            }
            0x122 => {
                self.player.money -= 250;
                true
            }
            0x123 => self.recruit_entity(self.npc_entity),
            0x124 => self.add_inventory_item(8, 0x12),
            0x125 => {
                self.player.money += 5;
                true
            }
            0x126 => {
                self.player.money += 10;
                true
            }
            0x127 => {
                self.player.money += 20;
                true
            }
            0x128 => {
                self.player.money += 50;
                true
            }
            0x129 => {
                self.player.money += 100;
                true
            }
            0x12A => {
                self.player.money += 250;
                true
            }
            0x12B => {
                self.player.available_xp = self.player.available_xp.saturating_add(10);
                true
            }
            0x12C => {
                self.player.available_xp = self.player.available_xp.saturating_add(20);
                true
            }
            0x12D => {
                self.player.available_xp = self.player.available_xp.saturating_add(50);
                true
            }
            0x12E => self
                .settlement()
                .map(|s| s.flags_4b2 & 0x10 != 0)
                .unwrap_or(false),
            0x12F => self.player.dialogue_id != 0xFF,
            0x130 => self
                .settlement()
                .map(|s| s.voivode() != 0)
                .unwrap_or(false),
            0x131 => self.player.money > 29,
            0x132 => self.player.money >= 40,
            0x133 => self.player.money >= 50,
            0x134 => self.player.money >= 100,
            0x135 => self.player.money >= 500,
            0x136 => {
                let level_high = self.npc().map(|e| e.stats.level > 3).unwrap_or(false);
                let mut threshold: i32 = if level_high { 500 } else { 300 };
                if self
                    .settlement()
                    .map(|s| s.flags_4b2 & 0x10 == 0)
                    .unwrap_or(true)
                {
                    threshold /= 2;
                }
                self.player.money >= threshold
            }
            0x137 => self
                .settlement()
                .map(|s| s.money_pool_10 != 0)
                .unwrap_or(false),
            0x138 => self.settlement().is_some(),
            0x139 => self.inventory_contains(9, 1),
            0x13A => self
                .settlement()
                .map(|s| s.producer(1) != 0)
                .unwrap_or(false),
            0x13B => self.target_state_is_3(1),
            0x13C => self.target_state_is_3(4),
            0x13D => self.target_state_is_3(6),
            0x13E => self.inventory_contains(9, 2),
            0x13F => self
                .settlement()
                .map(|s| s.producer(3) != 0)
                .unwrap_or(false),
            0x140 => self.target_state_is_3(3),
            0x141 => self
                .settlement()
                .map(|s| s.producer(2) != 0)
                .unwrap_or(false),
            0x142 => self.target_state_is_3(2),
            0x143 => self.target_state_is_3(5),
            0x144 => self.equipment_has_flag(1),
            0x145 => (self.player.interface_mode_d2 as i8) >= 0,
            0x146 => self.settlement_group_building() >= 5,
            0x147 => {
                let Some((base, count)) = self.target_base() else {
                    return false;
                };
                if count == 0 {
                    return false;
                }
                let index = base as usize + 7;
                self.target(index)
                    .map(|t| t.state != 0 || t.timer != 0)
                    .unwrap_or(false)
            }
            0x148 => {
                (0x151..=0x157).any(|op| self.exec_op(op))
            }
            0x149 => self.inventory_contains(9, 0),
            0x14A => self.player.hp_internal >= 1600,
            0x14B => {
                self.recalc_identification();
                true
            }
            0x14C => self.settle_into_settlement(self.npc_entity, Some(0)),
            0x14D => self
                .settlement()
                .map(|s| s.flags_4b2 & 0x20 != 0)
                .unwrap_or(false),
            0x14E => self.heal_target(false),
            0x14F => self.heal_target(true),
            0x150 => {
                let Some(s) = self.settlement() else {
                    return false;
                };
                if s.flags_4b2 & 0x10 == 0 || self.player.money < 100 {
                    return false;
                }
                // `FUN_00432804` is a pure predicate: it only reports whether
                // an eligible apprentice is available. It must not touch the
                // settlement group's activity.
                self.apprentice_candidate().is_some()
            }
            0x151 => self.requirement_predicate(1),
            0x152 => self.requirement_predicate(4),
            0x153 => self.requirement_predicate(6),
            0x154 => self.requirement_predicate(3),
            0x155 => self.requirement_predicate(2),
            0x156 => self.requirement_predicate(5),
            0x157 => {
                let Some((base, count)) = self.target_base() else {
                    return false;
                };
                if count == 0 {
                    return false;
                }
                self.requirement_predicate(base as usize + 7)
            }
            0x158 => {
                if self.inventory_contains(0x0B, 0x22) {
                    self.player.interface_mode_d2 = 0x13;
                    true
                } else {
                    false
                }
            }
            0x159 => self.prepare_ally(4),
            0x15A => self.prepare_ally(1),
            0x15B => self.prepare_ally(3),
            0x15C => self.prepare_ally(2),
            0x15D => {
                let capacity = crate::character::max_party(
                    self.player.class,
                    self.player.charisma_base,
                ) as usize;
                capacity <= 1 + self.party.len()
            }
            0x15E => self.npc_in_party(),
            0x15F => self.npc().map(|e| e.stats.hp_internal > 800).unwrap_or(false),
            0x160 => self.npc().map(|e| e.stats.level > 3).unwrap_or(false),
            0x161 => self
                .npc()
                .map(|e| e.archetype & 0x38 != 0)
                .unwrap_or(false),
            0x162 => self.prepare_apprentice(),
            0x163 => self.write_target_timer(1),
            0x164 => self.write_target_timer(4),
            0x165 => self.write_target_timer(6),
            0x166 => self.write_target_timer(3),
            0x167 => self.write_target_timer(2),
            0x168 => self.write_target_timer(5),
            0x169 => {
                let Some((base, count)) = self.target_base() else {
                    return false;
                };
                for i in 0..count as usize {
                    self.write_target_timer(base as usize + 7 + i);
                }
                true
            }
            0x16A => self.repair_party_gear(),
            0x16B => self.remove_world_entity(self.npc_entity),
            0x16C => {
                self.player.interface_mode_d2 = 0xFF;
                true
            }
            0x16D => {
                let Some(s) = self.settlement_mut() else {
                    return false;
                };
                s.flags_4b2 &= !0x15;
                s.progress_14 = 0;
                s.money_pool_10 = 0;
                true
            }
            0x16E => self.player.charisma_base > 60,
            _ => {
                self.unknown_ops.push(op);
                false
            }
        }
    }

    fn requirement_predicate(&mut self, target_index: usize) -> bool {
        let Some(s) = self.settlement() else {
            return false;
        };
        let Some(target) = s.targets.get(target_index).copied() else {
            return false;
        };
        if target.state != 0 || target.timer != 0 {
            return false;
        }
        let (required, _) = Self::requirement(target.key as usize);
        self.settlement_group_building() >= required
    }

    fn write_target_timer(&mut self, target_index: usize) -> bool {
        let Some(key) = self.target_key(target_index) else {
            return false;
        };
        let (_, value) = Self::requirement(key);
        let Some(s) = self.settlement_mut() else {
            return false;
        };
        let Some(target) = s.targets.get_mut(target_index) else {
            return false;
        };
        target.timer = value;
        true
    }

}

impl PlayableApp {
    /// Build a `SaveExtra` from the current runtime quest fields.
    pub fn quest_extra(&self) -> SaveExtra {
        let mut extra = SaveExtra {
            player_dialogue_id: self.player.dialogue_id,
            player_interface_mode_d2: self.player.interface_mode_d2,
            player_state_d3: self.player.state_d3,
            leader_entity_index: self.leader_entity_index as u32,
            ..SaveExtra::default()
        };
        extra.settlements = self
            .settlements
            .iter()
            .map(|s| crate::save::SettlementExtra {
                money_pool_10: s.money_pool_10,
                target_base0: s.target_base0,
                target_base1: s.target_base1,
            })
            .collect();
        extra.apprentice_start = self
            .settlements
            .iter()
            .map(|s| s.apprentice_start)
            .collect();
        if let Some(tables) = self.tables.as_ref() {
            extra.groups = tables
                .groups
                .iter()
                .map(|g| crate::save::GroupExtra {
                    activity_1d: g.activity_1d,
                    visited: g.visited,
                    growth_timer: g.growth_timer,
                    growth_countdown: g.growth_countdown,
                })
                .collect();
            // `Entity+0xD2` for every entity (`0x21` «Кукла»). Stored as a full
            // table because the authored value is not a fixed sentinel
            // (`Game.N` uses both `0` and `0xFF`).
            extra.npc_state_d2 = tables.entities.iter().map(|e| e.state_d2).collect();
        }
        extra
    }
}
