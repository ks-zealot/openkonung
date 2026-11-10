//! Class-11 item use (`FUN_00414DE4`).
//!
//! The original dispatches class-11 items by the instance subtype byte
//! (`item+0x03`) and applies one of ~50 effects (books, orbs, keys, quest
//! artifacts, the shovel, …). The full catalogue is recovered in
//! `CLASS11_ITEMS.md`.
//!
//! **C1:** subtype 0 (lost map, `use_map_item`).
//! **C2:** books/skill scrolls, identify, XP, wine, birch-bark letter, torch,
//! charisma and the relics state byte.
//! Later packages fill C3 (item manipulation) and C4 (world effects).
//!
//! Subtype 24 («Лопата») arms the dig cursor (`_DAT_0081D660 = 7`) and is not
//! consumed; carrying it is what allows buried WorldObject records (negative
//! money) to be collected on arrival (`FUN_00421B50` → `FUN_00433A8C(0x19)`).

use super::*;
use std::collections::HashSet;

use crate::combat::CombatTarget;
use crate::constants::timing::{WANDER_RADIUS_X, WANDER_RADIUS_Y};
use crate::inventory::{
    apply_entity_ops, apply_player_ops, insert_inventory_front, InventoryOwner, ItemLocation,
    StatOp,
};

impl PlayableApp {
    /// `FUN_00414DE4(actor, target_item, used_item)` for class-11 items.
    ///
    /// `owner` is the inventory owner whose item was used (`_DAT_0081D560`);
    /// `target` is the item under the cursor when a held item is applied to
    /// another item (`_DAT_0081D670`), `None` for a plain right-click use.
    /// Returns `true` only when the effect ran.
    pub fn use_class11_item(
        &mut self,
        owner: InventoryOwner,
        id: u16,
        target: Option<u16>,
    ) -> bool {
        let Some((11, subtype)) = self.world.res.item_class_subtype(id) else {
            return false;
        };
        let handled = match subtype {
            0 => self.use_map_item(id),

            // --- C2: books and self effects ---------------------------------
            1 => {
                self.apply_owner_ops(owner, &[StatOp::AddHealing(100)]);
                self.class11_message("Ты открыл для себя секреты древних целителей");
                self.consume_class11(owner, id);
                true
            }
            2 => {
                self.apply_owner_ops(owner, &[StatOp::AddBuilding(5)]);
                self.class11_message("Ты познал секреты строительного искусства");
                self.consume_class11(owner, id);
                true
            }
            3 => {
                self.apply_owner_ops(owner, &[StatOp::AddSmithing(4)]);
                self.class11_message("Ты познал секреты кузнечного дела");
                self.consume_class11(owner, id);
                true
            }
            4 => {
                self.apply_owner_ops(owner, &[StatOp::AddTrade(10)]);
                self.class11_message("Ты узнал много нового об искусстве торговли");
                self.consume_class11(owner, id);
                true
            }
            5 => {
                self.apply_owner_ops(owner, &[StatOp::AddIdentification(100)]);
                self.class11_message("Ты узнал много интересного о неведомых вещах");
                self.consume_class11(owner, id);
                true
            }
            8 => {
                self.apply_owner_ops(owner, &[StatOp::AddStrengthCurrent(3)]);
                self.consume_class11(owner, id);
                true
            }
            // `FUN_0041BA08`: clear the instance `+0x0E & 1` unidentified bit
            // on every owned item and add +1 leader identification per item.
            9 => {
                let identified = self.identify_owned_items(owner);
                self.player.identification = self
                    .player
                    .identification
                    .saturating_add(identified)
                    .min(100);
                // The original consumes the scroll even when every owned item
                // was already identified.
                self.consume_class11(owner, id);
                true
            }
            // `FUN_0041079C(user, 200)` (+ AUTO spend when the bit is set).
            10 => {
                self.apply_owner_ops(owner, &[StatOp::AddAvailableXp(200)]);
                if self.owner_auto_enabled(owner) {
                    self.auto_spend_xp(owner);
                }
                self.consume_class11(owner, id);
                true
            }
            12 => {
                let Some(snapshot) = self.snapshot(owner) else {
                    return false;
                };
                let n = (snapshot.dexterity_base as u32 / 3).min(5) as i32;
                let mut ops = Vec::with_capacity(7);
                if snapshot.temp_effect_timer == 0 {
                    ops.push(StatOp::SaveStats);
                }
                ops.push(StatOp::SetTempTimer((n.max(0) as u16).saturating_mul(30)));
                ops.push(StatOp::SetTempId(0));
                ops.push(StatOp::AddDexterityCurrent(-(n * 3)));
                ops.push(StatOp::AddCharismaCurrent(n));
                ops.push(StatOp::AddStrengthCurrent(n));
                ops.push(StatOp::AddEnduranceCurrent(n));
                self.apply_owner_ops(owner, &ops);
                self.consume_class11(owner, id);
                true
            }
            // `DAT_00615068 |= 0x80`: activate B7 quest slot 0.
            28 => {
                if let Some(slot) = self.quest_flags.get_mut(0) {
                    *slot |= 0x80;
                }
                self.class11_message("Ты прочитал Берестяную Грамоту");
                self.consume_class11(owner, id);
                true
            }
            // `_DAT_0081D628 = 1`: highlight ground markers until map load.
            // Reusable (the original does not consume the torch).
            31 => {
                self.torch_reveal = true;
                true
            }
            36 => {
                self.apply_owner_ops(owner, &[StatOp::AddCharismaCurrent(10)]);
                self.consume_class11(owner, id);
                true
            }
            // Quest opcode `0x114` and this item both set protagonist +0xD3.
            38 => {
                self.player.state_d3 = 0x46;
                self.consume_class11(owner, id);
                true
            }
            39 => {
                self.apply_owner_ops(owner, &[StatOp::AddCharismaCurrent(5)]);
                self.consume_class11(owner, id);
                true
            }

            // --- C3: item manipulation ---------------------------------------
            // Copy the target instance onto the held item (not consumed).
            7 => self.class11_duplicate(id, target),
            22 => self.class11_combine_sword(owner, id, target, 23),
            23 => self.class11_combine_sword(owner, id, target, 22),
            25 | 26 | 27 | 41 | 42 | 43 => self.class11_combine_amulet(owner, id, target),

            // `FUN_00414DE4` case 0x21 («Кукла»).
            33 => self.class11_doll(owner, id),

            // --- C4: world effects -------------------------------------------
            6 => self.class11_revive(owner, id),
            13 => self.class11_magnet(owner, id),
            14..=19 | 21 => self.class11_orb(owner, id, subtype),
            20 => self.class11_map_orb(owner, id),
            30 => self.class11_skeleton_lamp(owner, id),
            35 | 37 => self.class11_scatter(owner, id),
            44 | 52 | 53 => self.class11_key(owner, id, subtype),
            48 => self.class11_mirror(owner, id),

            // `FUN_00414DE4` case 0x18 («Лопата»): close the interface, arm the
            // dig cursor (`_DAT_0081D660 = 7`) and play sound 10. The item is
            // **not** consumed: the shovel is a carried requirement for buried
            // WorldObject records (`FUN_00433A8C(0x19)` in `FUN_00421B50`).
            24 => {
                // `_DAT_0081D610 = 0`: leave the interface (the shovel is used
                // from the character screen) before arming the dig cursor.
                self.character_screen = false;
                self.hovered_slot = None;
                self.dig_armed = true;
                true
            }

            // Any remaining subtype is a quest token with no use case.
            _ => false,
        };
        // `FUN_00414DE4` tail: every successful class-11 use plays sound 10
        // (subtype 0 already queues it from `use_map_item`).
        if handled && subtype != 0 {
            self.queue_sound(10);
        }
        handled
    }

    /// `FUN_00414DE4` case 7 («Дупликатор»): copy the target instance onto the
    /// used item. The used item is not consumed; it becomes the duplicate.
    fn class11_duplicate(&mut self, used: u16, target: Option<u16>) -> bool {
        let Some(target) = target else {
            return false;
        };
        if target == used || self.world.res.item_class(target).is_none() {
            return false;
        }
        self.world.res.copy_item_instance(target, used)
    }

    /// `FUN_00414DE4` cases 0x16/0x17: the used sword part must be applied to
    /// the other part (`other_subtype`); the target becomes the forged class-2
    /// subtype `0x1B` sword (120 durability), +30 XP, used part consumed.
    fn class11_combine_sword(
        &mut self,
        owner: InventoryOwner,
        used: u16,
        target: Option<u16>,
        other_subtype: u8,
    ) -> bool {
        let Some(target) = target else {
            return false;
        };
        if target == used || self.world.res.item_class_subtype(target) != Some((11, other_subtype))
        {
            return false;
        }
        let res = &mut self.world.res;
        if !res.set_item_class(target, 2)
            || !res.set_item_condition(target, -1)
            || !res.set_item_modifier(target, 0x0E)
            || !res.set_item_subtype(target, 0x1B)
            || !res.set_item_durability(target, 120.0)
            || !res.set_item_poison(target, 0)
        {
            return false;
        }
        self.apply_owner_ops(owner, &[StatOp::AddAvailableXp(30)]);
        if self.owner_auto_enabled(owner) {
            self.auto_spend_xp(owner);
        }
        self.consume_class11(owner, used);
        true
    }

    /// `FUN_00414DE4` cases 0x19/0x1A/0x1B and 0x29/0x2A/0x2B: combine two
    /// amulet parts. The target becomes the result subtype, +100 XP, used part
    /// consumed.
    fn class11_combine_amulet(
        &mut self,
        owner: InventoryOwner,
        used: u16,
        target: Option<u16>,
    ) -> bool {
        let Some(target) = target else {
            return false;
        };
        if target == used {
            return false;
        }
        let Some((11, used_subtype)) = self.world.res.item_class_subtype(used) else {
            return false;
        };
        let Some((11, target_subtype)) = self.world.res.item_class_subtype(target) else {
            return false;
        };
        let Some(result) = amulet_result(used_subtype, target_subtype) else {
            return false;
        };
        if !self.world.res.set_item_subtype(target, result) {
            return false;
        }
        self.apply_owner_ops(owner, &[StatOp::AddAvailableXp(100)]);
        if self.owner_auto_enabled(owner) {
            self.auto_spend_xp(owner);
        }
        self.consume_class11(owner, used);
        true
    }

    /// Apply a class-11 item that is currently on the cursor to the item at
    /// `target`. Ports the original held-item click path: on success the used
    /// item is either removed (consumed) or returned to the bag, and sound 10
    /// plays.
    pub(super) fn use_held_class11_on_item(
        &mut self,
        target_owner: InventoryOwner,
        target: ItemLocation,
    ) -> bool {
        let Some(held) = self.held_item else {
            return false;
        };
        if held.owner != target_owner {
            return false;
        }
        let Some(target_id) = self.slots(target_owner).and_then(|s| s.get(target)) else {
            return false;
        };
        if target_id == 0 || target_id == held.id {
            return false;
        }
        if !self.use_class11_item(held.owner, held.id, Some(target_id)) {
            return false;
        }
        self.held_item = None;
        let consumed = self.world.res.item_class(held.id) == Some(0xFF);
        if !consumed {
            let mut restored = false;
            if let Some(slots) = self.slots_mut(held.owner) {
                restored = insert_inventory_front(slots.inventory, held.id);
            }
            if !restored {
                self.set_pickup_message("Мешок у тебя не резиновый!");
            }
        }
        true
    }

    /// `FUN_00414DE4` case 0x21 («Кукла»): every living actor of every other
    /// group in the current location gets `Entity+0xD2 = 0x13`. Consumes the
    /// doll.
    ///
    /// The original walks the raw `Group[200]` table (no class-byte test), so
    /// inactive groups are included; it skips the user's own group id and
    /// actors whose action row (`Entity+0x00`) is `3`/`9` (death rows).
    fn class11_doll(&mut self, owner: InventoryOwner, id: u16) -> bool {
        let loc = self.current_map;
        let mut targets: Vec<usize> = Vec::new();
        if let Some(tables) = self.tables.as_ref() {
            for g in tables.groups.iter() {
                if g.index == 0 || g.loc_id as u32 != loc {
                    continue;
                }
                let first = g.first_entity as usize;
                let end = first.saturating_add(g.entity_count as usize);
                for idx in first..end {
                    if let Some(e) = tables.entities.get(idx) {
                        if e.archetype & 0x80 == 0 {
                            targets.push(idx);
                        }
                    }
                }
            }
        }

        for idx in targets {
            // Live action row: `motion.action` mirrors `Entity+0x00`.
            let action = self
                .population_cache
                .get(&loc)
                .and_then(|p| p.motion.get(&idx))
                .map(|m| m.action)
                .or_else(|| {
                    self.tables
                        .as_ref()
                        .and_then(|t| t.entities.get(idx))
                        .map(|e| e.kind)
                })
                .unwrap_or(0);
            if action == 3 || action == 9 {
                continue;
            }
            if let Some(tables) = self.tables.as_mut() {
                if let Some(e) = tables.entities.get_mut(idx) {
                    e.state_d2 = 0x13;
                }
            }
            if let Some(pop) = self.population_cache.get_mut(&loc) {
                if let Some(a) = pop.actors.iter_mut().find(|a| a.source_entity_id() == idx) {
                    a.entity.state_d2 = 0x13;
                }
            }
        }

        self.consume_class11(owner, id);
        true
    }

    /// `FUN_00414DE4` case 6: revive every dead actor in the current location
    /// (`FUN_00414ABC`). Consumes the words.
    fn class11_revive(&mut self, owner: InventoryOwner, id: u16) -> bool {
        let loc = self.current_map;
        let collision = self
            .world
            .loc(loc)
            .map(|l| l.collision.clone())
            .unwrap_or_default();

        let actors: Vec<(usize, (i32, i32), crate::population::Rect)> = self
            .population_cache
            .get(&loc)
            .map(|pop| {
                pop.actors
                    .iter()
                    .filter_map(|a| {
                        let m = pop.motion.get(&a.source_entity_id());
                        let dead =
                            m.map(|m| m.dead).unwrap_or(false) || a.entity.archetype & 0x80 != 0;
                        if !dead {
                            return None;
                        }
                        let cell = m
                            .map(|m| m.cell)
                            .unwrap_or((a.entity.x as i32, a.entity.y as i32));
                        let rect = m.map(|m| m.rect_a).unwrap_or_default();
                        Some((a.source_entity_id(), cell, rect))
                    })
                    .collect()
            })
            .unwrap_or_default();

        for (eid, cell, rect) in actors {
            let Some(new_cell) = self.revive_cell(rect, cell, &collision) else {
                continue;
            };
            let special = self
                .population_cache
                .get(&loc)
                .and_then(|p| p.actors.iter().find(|a| a.source_entity_id() == eid))
                .map(|a| a.entity.archetype == 0x4C)
                .unwrap_or(false);
            let roll = if special {
                (self.combat_rng.next_u32() % 5 + 1) as u8
            } else {
                0
            };
            if let Some(pop) = self.population_cache.get_mut(&loc) {
                let battle = pop
                    .actors
                    .iter()
                    .find(|a| a.source_entity_id() == eid)
                    .map(|a| crate::anim::battle_from_flags(a.entity.flags_b))
                    .unwrap_or(false);
                if let Some(m) = pop.motion.get_mut(&eid) {
                    m.cell = new_cell;
                    m.render_cell = new_cell;
                    m.dead = false;
                    m.walking = false;
                    m.path.clear();
                    m.target = None;
                    m.combat_target = None;
                    m.ranged_order = false;
                    m.ranged_fired = false;
                    m.action = crate::anim::idle_action(battle) as u8;
                    m.frame = 0;
                }
                if let Some(a) = pop.actors.iter_mut().find(|a| a.source_entity_id() == eid) {
                    a.entity.stats.hp_internal = 1600;
                    a.entity.stats.periodic_damage = 0;
                    a.entity.archetype &= 0x7F;
                    if special {
                        a.entity.raw_0x12 = roll;
                    }
                }
            }
        }

        // Party companions and the protagonist are group 0 and are revived too.
        for i in 0..self.party.len() {
            let (dead, cell, rect, special, battle) = {
                let m = &self.party[i];
                (
                    m.motion.dead || m.entity.archetype & 0x80 != 0,
                    m.motion.cell,
                    m.motion.rect_a,
                    m.entity.archetype == 0x4C,
                    crate::anim::battle_from_flags(m.entity.flags_b),
                )
            };
            if !dead {
                continue;
            }
            let Some(new_cell) = self.revive_cell(rect, cell, &collision) else {
                continue;
            };
            let roll = if special {
                (self.combat_rng.next_u32() % 5 + 1) as u8
            } else {
                0
            };
            if let Some(m) = self.party.get_mut(i) {
                m.motion.cell = new_cell;
                m.motion.render_cell = new_cell;
                m.motion.dead = false;
                m.motion.walking = false;
                m.motion.path.clear();
                m.motion.target = None;
                m.motion.combat_target = None;
                m.motion.ranged_order = false;
                m.motion.ranged_fired = false;
                m.motion.action = crate::anim::idle_action(battle) as u8;
                m.motion.frame = 0;
                m.entity.stats.hp_internal = 1600;
                m.entity.stats.periodic_damage = 0;
                m.entity.archetype &= 0x7F;
                if special {
                    m.entity.raw_0x12 = roll;
                }
            }
        }
        if self.player.dead {
            self.player.hp_internal = 1600;
            self.player.periodic_damage = 0;
            self.player.dead = false;
            self.player_attack_target = None;
            self.player_attack_frame = 0;
            self.player_death_frame = 0;
            self.player_active = false;
            self.player_path.clear();
        }

        self.consume_class11(owner, id);
        true
    }

    fn revive_cell(
        &self,
        rect: crate::population::Rect,
        cell: (i32, i32),
        collision: &[u16],
    ) -> Option<(i32, i32)> {
        let free = |x: i32, y: i32| {
            crate::loc::collision_is_free(collision, x, y) && !self.live_actor_cell(x, y)
        };
        if free(cell.0, cell.1) {
            return Some(cell);
        }
        let (x1, x2, y1, y2) = (
            rect.x1 as i32,
            rect.x2 as i32,
            rect.y1 as i32,
            rect.y2 as i32,
        );
        if x2 >= x1 && y2 >= y1 {
            for x in x1..=x2 {
                for y in y1..=y2 {
                    if free(x, y) {
                        return Some((x, y));
                    }
                }
            }
        }
        for radius in 1i32..=10 {
            for dx in -radius..=radius {
                for dy in -radius..=radius {
                    if dx.abs() != radius && dy.abs() != radius {
                        continue;
                    }
                    let x = cell.0 + dx;
                    let y = cell.1 + dy;
                    if free(x, y) {
                        return Some((x, y));
                    }
                }
            }
        }
        None
    }

    /// `FUN_00414DE4` case 13: move other groups' money to the leader and their
    /// inventory into the user's bag. Consumes the magnet.
    fn class11_magnet(&mut self, owner: InventoryOwner, id: u16) -> bool {
        let loc = self.current_map;
        let groups: Vec<(usize, i32, Vec<u16>)> = self
            .population_cache
            .get(&loc)
            .map(|pop| {
                pop.actors
                    .iter()
                    .filter(|a| a.entity.group_id != 0)
                    .map(|a| {
                        (
                            a.source_entity_id(),
                            a.entity.stats.money,
                            a.entity
                                .inventory
                                .iter()
                                .copied()
                                .filter(|&item| item != 0)
                                .collect::<Vec<u16>>(),
                        )
                    })
                    .filter(|(_, money, items)| *money != 0 || !items.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        for (eid, money, items) in groups {
            self.player.money = self.player.money.saturating_add(money);
            if let Some(slots) = self.slots_mut(owner) {
                for item in items {
                    if !insert_inventory_front(slots.inventory, item) {
                        break;
                    }
                }
            }
            if let Some(pop) = self.population_cache.get_mut(&loc) {
                if let Some(a) = pop.actors.iter_mut().find(|a| a.source_entity_id() == eid) {
                    a.entity.stats.money = 0;
                    a.entity.inventory.fill(0);
                }
            }
        }
        self.consume_class11(owner, id);
        true
    }

    /// `FUN_00414DE4` cases 14–19/21: teleport the party to a fixed location.
    /// The orbs are reusable.
    fn class11_orb(&mut self, _owner: InventoryOwner, _id: u16, subtype: u8) -> bool {
        let Some((dst, x, y, facing)) = orb_destination(subtype) else {
            return false;
        };
        let from = self.current_map;
        let _ = self.travel_to_map(from, dst, x, y, facing, 0);
        true
    }

    /// Case 20: open the world map (mode 5). Reusable.
    fn class11_map_orb(&mut self, _owner: InventoryOwner, _id: u16) -> bool {
        self.character_screen = false;
        self.quest_log_open = false;
        self.map_open = true;
        self.map_overlay = None;
        self.hovered_slot = None;
        self.queue_sound(4);
        true
    }

    /// Case 30: kill every other group's living actor in the location.
    fn class11_skeleton_lamp(&mut self, owner: InventoryOwner, id: u16) -> bool {
        let loc = self.current_map;
        let level = self.player.level;
        let ids: Vec<usize> = self
            .population_cache
            .get(&loc)
            .map(|pop| {
                pop.actors
                    .iter()
                    .filter(|a| {
                        a.entity.group_id != 0
                            && self.group_loc(a.source_group_id) != 0x53
                            && !pop
                                .motion
                                .get(&a.source_entity_id())
                                .map(|m| m.dead)
                                .unwrap_or(false)
                    })
                    .map(|a| a.source_entity_id())
                    .collect()
            })
            .unwrap_or_default();
        for eid in ids {
            self.kill_combatant(CombatTarget::World(eid), level, 0);
        }
        self.consume_class11(owner, id);
        true
    }

    /// Cases 35/37: order object-family actors to a random free cell on the
    /// edge of the wander rectangle (`FUN_004101E4` + `FUN_004142D4`). The
    /// whistle/pipe are reusable.
    fn class11_scatter(&mut self, _owner: InventoryOwner, _id: u16) -> bool {
        let loc = self.current_map;
        let collision = self
            .world
            .loc(loc)
            .map(|l| l.collision.clone())
            .unwrap_or_default();
        let actors: Vec<(usize, (i32, i32))> = self
            .population_cache
            .get(&loc)
            .map(|pop| {
                pop.actors
                    .iter()
                    .filter(|a| {
                        a.entity.archetype & 0x40 != 0
                            && !matches!(a.entity.archetype, 0x44 | 0x53 | 0x57 | 0x54)
                            && !pop
                                .motion
                                .get(&a.source_entity_id())
                                .map(|m| m.dead)
                                .unwrap_or(false)
                    })
                    .filter_map(|a| {
                        pop.motion
                            .get(&a.source_entity_id())
                            .map(|m| (a.source_entity_id(), m.cell))
                    })
                    .collect()
            })
            .unwrap_or_default();

        for (eid, cell) in actors {
            let edge = (self.combat_rng.next_u32() % 4) as u8;
            let Some(goal) = self.scatter_goal(cell, &collision, edge) else {
                continue;
            };
            if let Some(pop) = self.population_cache.get_mut(&loc) {
                if let Some(m) = pop.motion.get_mut(&eid) {
                    let mut occupied = HashSet::new();
                    Self::start_motion_walk(m, &collision, &mut occupied, goal);
                }
            }
        }
        true
    }

    fn scatter_goal(&self, cell: (i32, i32), collision: &[u16], edge: u8) -> Option<(i32, i32)> {
        let mut dx = match edge {
            0 => -WANDER_RADIUS_X * 2,
            1 => WANDER_RADIUS_X * 2,
            _ => -WANDER_RADIUS_X,
        };
        let mut dy = match edge {
            0 | 1 => -WANDER_RADIUS_Y,
            2 => -WANDER_RADIUS_Y * 2,
            _ => WANDER_RADIUS_Y * 2,
        };
        let steps = if edge < 2 {
            WANDER_RADIUS_Y * 2 + 1
        } else {
            WANDER_RADIUS_X * 2 + 1
        };
        for _ in 0..steps {
            let x = cell.0 + dx;
            let y = cell.1 + dy;
            if crate::loc::collision_is_free(collision, x, y) && !self.live_actor_cell(x, y) {
                return Some((x, y));
            }
            if edge < 2 {
                dy += 1;
            } else {
                dx += 1;
            }
        }
        None
    }

    /// Cases 44/52/53: conditional teleport at a fixed location cell. The keys
    /// are reusable and succeed (no effect) when used elsewhere.
    fn class11_key(&mut self, _owner: InventoryOwner, _id: u16, subtype: u8) -> bool {
        let Some((loc, cell, dst, x, y, facing)) = key_destination(subtype) else {
            return false;
        };
        if self.current_map != loc {
            return true;
        }
        let here = (self.player_x.round() as i32, self.player_y.round() as i32);
        if here != cell {
            return true;
        }
        let from = self.current_map;
        let _ = self.travel_to_map(from, dst, x, y, facing, 0);
        true
    }

    /// Case 48: flip negative money on current-location ground records
    /// (reactivate harvested markers) and rebuild the active list.
    fn class11_mirror(&mut self, owner: InventoryOwner, id: u16) -> bool {
        let loc = self.current_map;
        for source in &mut self.world_object_sources {
            if source.loc as u32 == loc && source.money < 0 {
                source.money = source.money.saturating_neg();
            }
        }
        for marker in &mut self.active_world_objects {
            if marker.money < 0 {
                marker.money = marker.money.saturating_neg();
            }
        }
        self.rebuild_world_objects();
        self.consume_class11(owner, id);
        true
    }

    fn group_loc(&self, group_id: usize) -> u8 {
        self.tables
            .as_ref()
            .and_then(|t| t.groups.get(group_id))
            .map(|g| g.loc_id)
            .unwrap_or(0)
    }

    fn class11_message(&mut self, text: &str) {
        self.set_pickup_message(text);
    }

    fn apply_owner_ops(&mut self, owner: InventoryOwner, ops: &[StatOp]) {
        match owner {
            InventoryOwner::Protagonist => apply_player_ops(&mut self.player, ops),
            InventoryOwner::Party(i) => {
                if let Some(m) = self.party.get_mut(i) {
                    apply_entity_ops(&mut m.entity, ops);
                }
            }
        }
    }

    fn owner_auto_enabled(&self, owner: InventoryOwner) -> bool {
        match owner {
            InventoryOwner::Protagonist => self.player.auto_enabled(),
            InventoryOwner::Party(i) => self
                .party
                .get(i)
                .map(|m| m.entity.flags_b & 0x02 != 0)
                .unwrap_or(false),
        }
    }

    /// Mark the used item consumed (`class = 0xFF`) and compact the owner's
    /// containers. Mirrors `*used = -1` plus the inventory sweep.
    fn consume_class11(&mut self, owner: InventoryOwner, id: u16) {
        self.world.res.set_item_class(id, 0xFF);
        if let Some(mut slots) = self.slots_mut(owner) {
            slots.remove_id(id);
        }
    }

    /// `FUN_0041BA08`: iterate equipment, special, inventory and additional
    /// slots; clear the instance `+0x0E` bit 0 on every unidentified item and
    /// return how many were identified.
    fn identify_owned_items(&mut self, owner: InventoryOwner) -> u16 {
        let ids: Vec<u16> = match self.slots(owner) {
            Some(slots) => slots
                .equipment
                .iter()
                .chain(std::iter::once(&slots.special))
                .chain(slots.inventory.iter())
                .chain(slots.additional.iter())
                .copied()
                .filter(|&id| id != 0)
                .collect(),
            None => return 0,
        };
        let mut identified = 0u16;
        for id in ids {
            let Some(instance) = self.world.res.item_instance(id) else {
                continue;
            };
            if instance.flags & 1 == 0 {
                continue;
            }
            if self.world.res.set_item_flags(id, instance.flags & !1) {
                identified = identified.saturating_add(1);
            }
        }
        identified
    }
}

/// `FUN_00410010` amulet-part pair → result subtype (0x28 full amulet).
fn amulet_result(used: u8, target: u8) -> Option<u8> {
    match (used, target) {
        (25, 26) | (26, 25) => Some(41),
        (25, 27) | (27, 25) => Some(42),
        (25, 43) | (43, 25) => Some(40),
        (26, 27) | (27, 26) => Some(43),
        (26, 42) | (42, 26) => Some(40),
        (27, 41) | (41, 27) => Some(40),
        _ => None,
    }
}

/// Fixed orb destinations: (location, x, y, facing).
fn orb_destination(subtype: u8) -> Option<(u32, i32, i32, i32)> {
    match subtype {
        14 => Some((28, 184, 18, 1)),
        15 => Some((5, 60, 68, 2)),
        16 => Some((10, 147, 66, 1)),
        17 => Some((20, 49, 41, 2)),
        18 => Some((3, 141, 54, 1)),
        19 => Some((40, 42, 31, 1)),
        21 => Some((43, 18, 24, 2)),
        _ => None,
    }
}

/// Conditional key teleports: (required location, required cell, destination
/// location, x, y, facing).
fn key_destination(subtype: u8) -> Option<(u32, (i32, i32), u32, i32, i32, i32)> {
    match subtype {
        44 => Some((50, (34, 11), 46, 68, 13, 2)),
        52 => Some((48, (23, 9), 49, 106, 17, 2)),
        53 => Some((49, (15, 17), 50, 134, 12, 2)),
        _ => None,
    }
}
