//! Inventory integration for [`PlayableApp`]: owner switching, cursor lift /
//! place, crafting, potion use, ammo stacking and ground drops.

use super::*;
use crate::inventory::*;
use crate::world_objects::{WORLD_OBJECT_GENERIC_SPRITE, WORLD_OBJECT_ITEM_SLOTS};

fn slots_ref_parts<'a>(
    player: &'a crate::character::PlayerCharacter,
    party: &'a [crate::actors::PartyMember],
    owner: InventoryOwner,
) -> Option<SlotsRef<'a>> {
    match owner {
        InventoryOwner::Protagonist => Some(SlotsRef {
            equipment: &player.equipment,
            inventory: &player.inventory,
            additional: &player.additional_equipment,
            special: player.special_equipment,
        }),
        InventoryOwner::Party(i) => party.get(i).map(|m| SlotsRef {
            equipment: &m.entity.equipment,
            inventory: &m.entity.inventory,
            additional: &m.entity.additional_equipment,
            special: m.entity.special_equipment,
        }),
    }
}

fn slots_mut_parts<'a>(
    player: &'a mut crate::character::PlayerCharacter,
    party: &'a mut [crate::actors::PartyMember],
    owner: InventoryOwner,
) -> Option<SlotsMut<'a>> {
    match owner {
        InventoryOwner::Protagonist => Some(SlotsMut {
            equipment: &mut player.equipment,
            inventory: &mut player.inventory,
            additional: &mut player.additional_equipment,
            special: &mut player.special_equipment,
        }),
        InventoryOwner::Party(i) => party.get_mut(i).map(|m| SlotsMut {
            equipment: &mut m.entity.equipment,
            inventory: &mut m.entity.inventory,
            additional: &mut m.entity.additional_equipment,
            special: &mut m.entity.special_equipment,
        }),
    }
}

impl PlayableApp {
    /// Number of editable owners (protagonist + recruited companions).
    pub fn owner_count(&self) -> usize {
        1 + self.party.len()
    }

    /// Map a HUD party cell to an owner (`0` = protagonist).
    pub fn owner_for_cell(&self, cell: usize) -> Option<InventoryOwner> {
        if cell == 0 {
            Some(InventoryOwner::Protagonist)
        } else {
            self.party
                .get(cell - 1)
                .map(|_| InventoryOwner::Party(cell - 1))
        }
    }

    pub fn owner_index(&self, owner: InventoryOwner) -> Option<usize> {
        match owner {
            InventoryOwner::Protagonist => Some(0),
            InventoryOwner::Party(i) => {
                if i < self.party.len() {
                    Some(i + 1)
                } else {
                    None
                }
            }
        }
    }

    pub fn slots(&self, owner: InventoryOwner) -> Option<SlotsRef<'_>> {
        slots_ref_parts(&self.player, &self.party, owner)
    }

    pub fn slots_mut(&mut self, owner: InventoryOwner) -> Option<SlotsMut<'_>> {
        slots_mut_parts(&mut self.player, &mut self.party, owner)
    }

    pub fn snapshot(&self, owner: InventoryOwner) -> Option<OwnerSnapshot> {
        match owner {
            InventoryOwner::Protagonist => Some(OwnerSnapshot::from_player(&self.player)),
            InventoryOwner::Party(i) => self
                .party
                .get(i)
                .map(|m| OwnerSnapshot::from_entity(&m.entity)),
        }
    }

    /// Minimal sheet used by the character-screen raise predicates.
    pub(crate) fn growth_sheet(&self, owner: InventoryOwner) -> Option<crate::character_screen::CharacterSheet> {
        Some(crate::character_screen::CharacterSheet {
            class: self.owner_class(owner),
            snapshot: self.snapshot(owner)?,
            ..Default::default()
        })
    }

    /// Stat-raise action (`FUN_0042260C` commands 2/3/5/6): the cost from
    /// `FUN_00410B08` is moved from `+0x38` available XP to `+0x3C` invested XP
    /// and the **current** stat half is incremented by one.
    pub fn raise_owner_stat(
        &mut self,
        owner: InventoryOwner,
        stat: crate::character::PrimaryStat,
    ) -> bool {
        let Some(sheet) = self.growth_sheet(owner) else {
            return false;
        };
        let cost = crate::character_screen::stat_raise_cost(&sheet, stat);
        if cost == 0 {
            return false;
        }
        match owner {
            InventoryOwner::Protagonist => {
                self.player.available_xp = self.player.available_xp.saturating_sub(cost);
                self.player.invested_xp = self.player.invested_xp.saturating_add(cost);
                let slot = match stat {
                    crate::character::PrimaryStat::Charisma => &mut self.player.charisma_current,
                    crate::character::PrimaryStat::Strength => &mut self.player.strength_current,
                    crate::character::PrimaryStat::Dexterity => &mut self.player.dexterity_current,
                    crate::character::PrimaryStat::Endurance => &mut self.player.endurance_current,
                };
                *slot = slot.saturating_add(1);
            }
            InventoryOwner::Party(i) => {
                let Some(m) = self.party.get_mut(i) else {
                    return false;
                };
                let s = &mut m.entity.stats;
                s.available_xp = s.available_xp.saturating_sub(cost);
                s.invested_xp = s.invested_xp.saturating_add(cost);
                let slot = match stat {
                    crate::character::PrimaryStat::Charisma => &mut s.charisma_current,
                    crate::character::PrimaryStat::Strength => &mut s.strength_current,
                    crate::character::PrimaryStat::Dexterity => &mut s.dexterity_current,
                    crate::character::PrimaryStat::Endurance => &mut s.endurance_current,
                };
                *slot = slot.saturating_add(1);
            }
        }
        true
    }

    /// Level-up action (`FUN_0042260C` command 0): gated by
    /// `FUN_00410B08(entity, 0)` and then `Entity+0x33 += 1` (no XP cost).
    pub fn level_up_owner(&mut self, owner: InventoryOwner) -> bool {
        let Some(sheet) = self.growth_sheet(owner) else {
            return false;
        };
        if !crate::character_screen::level_up_available(&sheet) {
            return false;
        }
        match owner {
            InventoryOwner::Protagonist => {
                self.player.level = self.player.level.wrapping_add(1);
            }
            InventoryOwner::Party(i) => {
                if let Some(m) = self.party.get_mut(i) {
                    m.entity.stats.level = m.entity.stats.level.wrapping_add(1);
                } else {
                    return false;
                }
            }
        }
        true
    }

    /// Current owner's cell in the runtime grid.
    pub fn owner_cell_pos(&self, owner: InventoryOwner) -> (i32, i32) {
        match owner {
            InventoryOwner::Protagonist => {
                (self.player_x.round() as i32, self.player_y.round() as i32)
            }
            InventoryOwner::Party(i) => self.party.get(i).map(|m| m.motion.cell).unwrap_or((0, 0)),
        }
    }

    pub fn owner_label(&self, owner: InventoryOwner) -> String {
        match owner {
            InventoryOwner::Protagonist => self.player.hero.name.to_string(),
            InventoryOwner::Party(i) => self
                .party
                .get(i)
                .map(|m| {
                    self.world
                        .res
                        .character_name(m.entity.name_id)
                        .unwrap_or_else(|| format!("Member {}", i + 1))
                })
                .unwrap_or_else(|| "Member".to_string()),
        }
    }

    pub fn owner_class(&self, owner: InventoryOwner) -> u8 {
        match owner {
            InventoryOwner::Protagonist => self.player.entity_class(),
            InventoryOwner::Party(i) => self
                .party
                .get(i)
                .map(|m| m.entity.archetype & 0x07)
                .unwrap_or(0),
        }
    }

    pub fn owner_visual(&self, owner: InventoryOwner) -> Option<(u8, u32)> {
        match owner {
            InventoryOwner::Protagonist => Some((
                self.player.hero.visual_index,
                self.player.hero.palette_offset,
            )),
            InventoryOwner::Party(i) => self
                .party
                .get(i)
                .map(|m| (m.entity.visual_index, m.entity.palette_offset)),
        }
    }

    pub fn owner_raw_0x12(&self, owner: InventoryOwner) -> u8 {
        match owner {
            InventoryOwner::Protagonist => self.player.raw_0x12,
            InventoryOwner::Party(i) => self.party.get(i).map(|m| m.entity.raw_0x12).unwrap_or(0),
        }
    }

    pub fn owner_flags_b(&self, owner: InventoryOwner) -> u8 {
        match owner {
            InventoryOwner::Protagonist => self.player.flags_b,
            InventoryOwner::Party(i) => self.party.get(i).map(|m| m.entity.flags_b).unwrap_or(0),
        }
    }

    pub fn owner_equipment(&self, owner: InventoryOwner) -> [u16; 6] {
        self.slots(owner).map(|s| *s.equipment).unwrap_or([0; 6])
    }

    pub fn owner_inventory(&self, owner: InventoryOwner) -> [u16; 32] {
        self.slots(owner).map(|s| *s.inventory).unwrap_or([0; 32])
    }

    pub fn set_inventory_owner(&mut self, owner: InventoryOwner) {
        self.inventory_owner = owner;
        self.inventory_scroll = 0;
        self.hovered_slot = None;
    }

    fn owner_healing(&mut self, owner: InventoryOwner) -> Option<&mut u16> {
        match owner {
            InventoryOwner::Protagonist => Some(&mut self.player.healing),
            InventoryOwner::Party(i) => self.party.get_mut(i).map(|m| &mut m.entity.stats.healing),
        }
    }

    pub fn sweep_owner(&mut self, owner: InventoryOwner) {
        let broken: Vec<u16> = {
            let Some(slots) = self.slots(owner) else {
                return;
            };
            slots
                .equipment
                .iter()
                .chain(slots.additional.iter())
                .chain(slots.inventory.iter())
                .chain(std::iter::once(&slots.special))
                .copied()
                .filter(|&id| id != 0 && is_broken(&self.world.res, id))
                .collect()
        };
        if broken.is_empty() {
            return;
        }
        if let Some(mut slots) = self.slots_mut(owner) {
            for id in &broken {
                slots.remove_id(*id);
            }
        }
        self.set_pickup_message("Сломанный предмет выброшен");
    }

    pub fn sweep_all_owners(&mut self) {
        self.sweep_owner(InventoryOwner::Protagonist);
        for i in 0..self.party.len() {
            self.sweep_owner(InventoryOwner::Party(i));
        }
    }

    /// Lift `loc` from the current owner onto the cursor.
    pub fn lift_item(&mut self, loc: ItemLocation) -> bool {
        if self.held_item.is_some() {
            return self.deposit_held();
        }
        let owner = self.inventory_owner;
        let Some(mut slots) = self.slots_mut(owner) else {
            return false;
        };
        let Some(id) = slots.take(loc) else {
            return false;
        };
        if id == 0 {
            return false;
        }
        self.held_item = Some(HeldItem {
            id,
            owner,
            source: Some(loc),
        });
        self.queue_sound(4);
        true
    }

    /// Deposit the held item into the current owner's inventory (original
    /// `FUN_00424410`): stack ammo first, then weight and free-slot checks,
    /// then insert at slot 0.
    pub fn deposit_held(&mut self) -> bool {
        let Some(held) = self.held_item else {
            return false;
        };
        let target = self.inventory_owner;
        let cap = self
            .snapshot(target)
            .map(|s| capacity(s.endurance_base))
            .unwrap_or(0);
        let mut handled = false;
        let mut message: Option<&'static str> = None;
        {
            let PlayableApp {
                world,
                player,
                party,
                ..
            } = self;
            let Some(mut slots) = slots_mut_parts(player, party, target) else {
                return false;
            };
            if try_stack_ammo(&mut world.res, &mut slots, held.id) {
                handled = true;
            } else {
                let carried = carried_weight(&world.res, &slots.as_ref());
                let w = item_weight(&world.res, held.id);
                if cap < carried.saturating_add(w) {
                    message = Some("Превышен вес");
                } else if insert_inventory_front(slots.inventory, held.id) {
                    handled = true;
                } else {
                    message = Some("Мешок у тебя не резиновый!");
                }
            }
        }
        if let Some(m) = message {
            self.set_pickup_message(m);
        }
        if handled {
            self.held_item = None;
            self.queue_sound(4);
            self.sweep_owner(target);
        }
        handled
    }

    /// Place the held item into a concrete container. Inventory destinations
    /// are faithful front-inserts; equipment destinations apply class gates and
    /// requirements and push a displaced item to the inventory front.
    pub fn place_held(&mut self, dest: ItemLocation) -> bool {
        let Some(held) = self.held_item else {
            return false;
        };
        if matches!(dest, ItemLocation::Inventory(_)) {
            return self.deposit_held();
        }
        let owner = self.inventory_owner;
        let Some(snapshot) = self.snapshot(owner) else {
            return false;
        };
        let Some((class, _subtype)) = self.world.res.item_class_subtype(held.id) else {
            return false;
        };
        let mut message: Option<&'static str> = None;
        let mut moved = false;
        let mut displaced_class: Option<u8> = None;
        {
            let PlayableApp {
                world,
                player,
                party,
                ..
            } = self;
            let Some(mut slots) = slots_mut_parts(player, party, owner) else {
                return false;
            };
            let gate_ok = match dest {
                ItemLocation::Main(slot) => main_slot_accepts(class, slot),
                ItemLocation::Additional(slot) => {
                    additional_slot_for_class(class, 0x0E + slot as u16) == Some(slot)
                }
                ItemLocation::Special => true,
                ItemLocation::Inventory(_) => true,
            };
            if !gate_ok || !requirement_ok(&snapshot, &world.res, &slots.as_ref(), held.id) {
                message = Some("Не подходит");
            } else {
                let displaced = slots.set(dest, held.id);
                moved = true;
                if let Some(old) = displaced {
                    if old != 0 {
                        displaced_class = world.res.item_class(old);
                        if try_stack_ammo(&mut world.res, &mut slots, old) {
                            // consumed by merge
                        } else if !insert_inventory_front(slots.inventory, old) {
                            slots.set(dest, old);
                            moved = false;
                            message = Some("Мешок у тебя не резиновый!");
                        }
                    }
                }
            }
        }
        if let Some(m) = message {
            self.set_pickup_message(m);
        }
        if moved {
            self.held_item = None;
            let sound = match dest {
                ItemLocation::Main(_) => {
                    crate::sound::events::equip_main(class, displaced_class)
                }
                ItemLocation::Additional(_) | ItemLocation::Special => {
                    crate::sound::events::EQUIP_OTHER
                }
                ItemLocation::Inventory(_) => crate::sound::events::UI_CLICK,
            };
            self.queue_sound(sound);
            self.sweep_owner(owner);
        }
        moved
    }

    /// Drop the held item onto the ground at its source owner's cell
    /// (`FUN_004138C8`).
    pub fn drop_held_on_ground(&mut self) -> bool {
        let Some(held) = self.held_item else {
            return false;
        };
        let cell = self.owner_cell_pos(held.owner);
        self.add_ground_item(cell, held.id);
        self.held_item = None;
        self.queue_sound(4);
        true
    }

    /// Append one item to a runtime ground marker (`FUN_004138C8`, max 22).
    pub fn add_ground_item(&mut self, cell: (i32, i32), id: u16) {
        if id == 0 {
            return;
        }
        let x = cell.0.clamp(0, u16::MAX as i32) as u16;
        let y = cell.1.clamp(0, u16::MAX as i32) as u16;
        let loc = self.current_map as u16;
        let mut merged = false;
        if let Some(existing) = self
            .world_object_sources
            .iter_mut()
            .find(|s| s.loc == loc && s.x == x && s.y == y)
        {
            if let Some(slot) = existing.items.iter_mut().find(|v| **v == 0) {
                *slot = id;
                merged = true;
            }
        }
        if merged {
            self.rebuild_world_objects();
            return;
        }
        let mut items = [0u16; WORLD_OBJECT_ITEM_SLOTS];
        items[0] = id;
        let index = self.world_object_sources.len();
        self.world_object_sources.push(WorldObjectSource {
            index,
            t0: 0,
            t4: 0,
            x,
            y,
            money: 0,
            loc,
            items,
        });
        let _ = WORLD_OBJECT_GENERIC_SPRITE;
        self.rebuild_world_objects();
    }

    /// Give the held item to the party member in `cell` (`FUN_00424410`).
    pub fn give_held_to_member(&mut self, cell: usize) -> bool {
        let Some(held) = self.held_item else {
            return false;
        };
        let Some(target) = self.owner_for_cell(cell) else {
            return false;
        };
        if target == held.owner {
            return self.deposit_held();
        }
        let cap = self
            .snapshot(target)
            .map(|s| capacity(s.endurance_base))
            .unwrap_or(0);
        let mut ok = false;
        let mut merged = false;
        let mut message: Option<&'static str> = None;
        {
            let PlayableApp {
                world,
                player,
                party,
                ..
            } = self;
            let Some(mut slots) = slots_mut_parts(player, party, target) else {
                return false;
            };
            if try_stack_ammo(&mut world.res, &mut slots, held.id) {
                ok = true;
                merged = true;
            } else {
                let carried = carried_weight(&world.res, &slots.as_ref());
                let w = item_weight(&world.res, held.id);
                if cap < carried.saturating_add(w) {
                    message = Some("Превышен вес");
                } else if insert_inventory_front(slots.inventory, held.id) {
                    ok = true;
                } else {
                    message = Some("Мешок у тебя не резиновый!");
                }
            }
        }
        if let Some(m) = message {
            self.set_pickup_message(m);
        }
        if ok {
            self.held_item = None;
            self.queue_sound(4);
            if !merged {
                self.sweep_owner(target);
            }
        }
        ok
    }

    /// Right-click an inventory slot: original class switch. Class 0..=5 equips
    /// into the matching main slot, class 6 into the first additional slot,
    /// class 12 into the special (ammo) slot, class 9 uses the item.
    pub fn right_click_inventory_slot(&mut self, visible_slot: usize) -> bool {
        if self.held_item.is_some() {
            return false;
        }
        let idx = self.inventory_scroll + visible_slot;
        let Some(id) = self
            .slots(self.inventory_owner)
            .and_then(|s| s.inventory.get(idx).copied())
        else {
            return false;
        };
        if id == 0 {
            return false;
        }
        let Some(class) = self.world.res.item_class(id) else {
            return false;
        };
        let dest = match class {
            0..=5 => ItemLocation::Main(class as usize),
            6 => ItemLocation::Additional(0),
            7 => ItemLocation::Additional(1),
            8 => ItemLocation::Additional(3),
            12 => ItemLocation::Special,
            9 => return self.use_inventory_slot(visible_slot),
            11 => return self.use_map_item(id),
            _ => return false,
        };
        if !self.lift_item(ItemLocation::Inventory(idx)) {
            return false;
        }
        if self.place_held(dest) {
            true
        } else {
            self.deposit_held();
            false
        }
    }

    /// Right-click a class-11 / subtype-0 map item (`FUN_00414DE4` case 0):
    /// reveal one random undiscovered, non-reserved world-map location and
    /// consume the item (`class = 0xFF`). `FUN_0043EA46` is the CRT `rand()`.
    pub fn use_map_item(&mut self, id: u16) -> bool {
        if self.world.res.item_class_subtype(id) != Some((11, 0)) {
            return false;
        }
        let start = (self.combat_rng.next_u32() % 56) as usize;
        let mut found = false;
        for step in 0..56 {
            let i = (start + step) % 56;
            if self.discovered_locations[i] == 0
                && crate::world_map::MAP_RESERVED[i] == 0
            {
                self.discovered_locations[i] = 1;
                found = true;
                break;
            }
        }
        if !found {
            return false;
        }
        self.map_overlay = None;
        self.world.res.set_item_class(id, 0xFF);
        self.sweep_all_owners();
        self.queue_sound(crate::sound::events::MAP_ITEM);
        true
    }


    /// Right-click an inventory slot: use a class-9 item in place.
    pub fn use_inventory_slot(&mut self, visible_slot: usize) -> bool {
        let owner = self.inventory_owner;
        let Some(slots) = self.slots(owner) else {
            return false;
        };
        let idx = self.inventory_scroll + visible_slot;
        let Some(&id) = slots.inventory.get(idx) else {
            return false;
        };
        if id == 0 {
            return false;
        }
        let Some(snapshot) = self.snapshot(owner) else {
            return false;
        };
        let ops = match use_item(&mut self.world.res, &snapshot, id) {
            Ok(ops) => ops,
            Err(_) => return false,
        };
        match owner {
            InventoryOwner::Protagonist => apply_player_ops(&mut self.player, &ops),
            InventoryOwner::Party(i) => {
                if let Some(m) = self.party.get_mut(i) {
                    apply_entity_ops(&mut m.entity, &ops);
                }
            }
        }
        self.queue_sound(9);
        true
    }

    /// Apply a held class-9 item to another item (`FUN_0041DAB0` target path).
    /// Both items must belong to the same owner. The used item remains in the
    /// inventory transformed by the use (oil -> empty jar) and the target is
    /// modified in place.
    pub fn use_held_on_item(
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
        let Some(snapshot) = self.snapshot(held.owner) else {
            return false;
        };
        let ops = match use_item_on(&mut self.world.res, &snapshot, held.id, target_id) {
            Ok(ops) => ops,
            Err(_) => return false,
        };
        match held.owner {
            InventoryOwner::Protagonist => apply_player_ops(&mut self.player, &ops),
            InventoryOwner::Party(i) => {
                if let Some(m) = self.party.get_mut(i) {
                    apply_entity_ops(&mut m.entity, &ops);
                }
            }
        }
        // Drop the cursor and put the transformed used item back into the bag
        // (front insert; the original leaves it in its existing slot).
        self.held_item = None;
        let mut restored = false;
        if let Some(slots) = self.slots_mut(held.owner) {
            restored = insert_inventory_front(slots.inventory, held.id);
        }
        if !restored {
            self.set_pickup_message("Мешок у тебя не резиновый!");
        }
        self.queue_sound(9);
        self.sweep_owner(held.owner);
        true
    }

    /// If both the held item and the item at `target` are class 9, run the
    /// alchemy merge; otherwise try the target-item use path (`Масло`/`Яд`).
    pub fn try_craft_held_at(
        &mut self,
        target_owner: InventoryOwner,
        target: ItemLocation,
    ) -> bool {
        let Some(held) = self.held_item else {
            return false;
        };
        let Some(target_id) = self.slots(target_owner).and_then(|s| s.get(target)) else {
            return false;
        };
        if target_id == 0 || target_id == held.id {
            return false;
        }
        let held_class = self.world.res.item_class(held.id);
        let target_class = self.world.res.item_class(target_id);
        match (held_class, target_class) {
            (Some(9), Some(9)) => self.craft_held_with(target_owner, target),
            (Some(9), Some(_)) => self.use_held_on_item(target_owner, target),
            _ => false,
        }
    }

    /// Craft the held class-9 item with a target item (`FUN_0041BB70`).
    pub fn craft_held_with(&mut self, target_owner: InventoryOwner, target: ItemLocation) -> bool {
        let Some(held) = self.held_item else {
            return false;
        };
        let Some(mut target_slots) = self.slots_mut(target_owner) else {
            return false;
        };
        let Some(target_id) = target_slots.get(target) else {
            return false;
        };
        if target_id == 0 || target_id == held.id {
            return false;
        }
        target_slots.take(target);
        drop(target_slots);
        let mut healing = self.snapshot(target_owner).map(|s| s.healing).unwrap_or(0);
        let Some(result) = craft(&mut self.world.res, &mut healing, held.id, target_id) else {
            return false;
        };
        if let Some(h) = self.owner_healing(target_owner) {
            *h = healing;
        }
        if result.container != 0 {
            let broken = self
                .world
                .res
                .item_class(result.container)
                .map(|c| c == 0xFF)
                .unwrap_or(true);
            if !broken {
                let mut message = false;
                if let Some(slots) = self.slots_mut(target_owner) {
                    if !insert_inventory_front(slots.inventory, result.container) {
                        message = true;
                    }
                }
                if message {
                    self.set_pickup_message("Мешок у тебя не резиновый!");
                }
            }
        }
        self.queue_sound(9);
        self.sweep_owner(target_owner);
        true
    }

    fn party_cell_at_virtual(&self, vx: i32, vy: i32) -> Option<usize> {
        let l = crate::hud::layout(VIRTUAL_W);
        (0..crate::hud::PARTY_CELLS)
            .find(|&i| l.party_cell(i).map(|r| r.contains(vx, vy)).unwrap_or(false))
    }

    /// Finish a drag at the current virtual cursor position.
    pub fn drop_held_at_cursor(&mut self) -> bool {
        if self.held_item.is_none() {
            return false;
        }
        let (vx, vy) = self.cursor_virtual;
        let (vx, vy) = (vx.round() as i32, vy.round() as i32);
        if let Some(cell) = self.party_cell_at_virtual(vx, vy) {
            if self.character_screen {
                self.select_party_member(cell);
                if let Some(owner) = self.owner_for_cell(cell) {
                    self.set_inventory_owner(owner);
                }
                return self.deposit_held();
            }
            return self.give_held_to_member(cell);
        }
        if self.character_screen {
            let equipment = self.owner_equipment(self.inventory_owner);
            let raw = self.owner_raw_0x12(self.inventory_owner);
            let hit = crate::character_screen::hit_test(vx, vy, VIRTUAL_W, &equipment, raw);
            return match hit {
                crate::character_screen::ScreenHit::Main(slot) => {
                    let loc = ItemLocation::Main(slot);
                    self.try_craft_held_at(self.inventory_owner, loc) || self.place_held(loc)
                }
                crate::character_screen::ScreenHit::Additional(slot) => {
                    let loc = ItemLocation::Additional(slot);
                    self.try_craft_held_at(self.inventory_owner, loc) || self.place_held(loc)
                }
                crate::character_screen::ScreenHit::Special => {
                    let loc = ItemLocation::Special;
                    self.try_craft_held_at(self.inventory_owner, loc) || self.place_held(loc)
                }
                crate::character_screen::ScreenHit::Inventory(slot) => {
                    let idx = self.inventory_scroll + slot;
                    let loc = ItemLocation::Inventory(idx);
                    self.try_craft_held_at(self.inventory_owner, loc) || self.deposit_held()
                }
                crate::character_screen::ScreenHit::RaiseLevel
                | crate::character_screen::ScreenHit::Raise(_) => self.deposit_held(),
                crate::character_screen::ScreenHit::None => self.deposit_held(),
            };
        }
        let l = crate::hud::layout(VIRTUAL_W);
        for slot in 0..l.visible_slots {
            if let Some(r) = l.inventory_slot_rect(slot, 70) {
                if r.contains(vx, vy) {
                    let idx = self.inventory_scroll + slot;
                    let loc = ItemLocation::Inventory(idx);
                    return self.try_craft_held_at(self.inventory_owner, loc)
                        || self.deposit_held();
                }
            }
        }
        // Dropped over the world: original `FUN_004138C8` ground drop.
        self.drop_held_on_ground()
    }

    /// Advance temporary potion timers at the original 60 ms logic rate.
    pub fn tick_temp_effects(&mut self, dt: f64) {
        self.temp_effect_accum += dt.max(0.0) * LOGIC_TICK_HZ;
        let frames = self.temp_effect_accum.floor() as u32;
        if frames == 0 {
            return;
        }
        self.temp_effect_accum -= frames as f64;
        for _ in 0..frames {
            crate::inventory::tick_player_temp(&mut self.player);
            for m in self.party.iter_mut() {
                crate::inventory::tick_entity_temp(&mut m.entity);
            }
        }
    }
}

