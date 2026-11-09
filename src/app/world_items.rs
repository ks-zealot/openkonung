//! Runtime Game.N WorldObject integration: location filtering, rendering
//! sprite resolution and the original herb pickup/respawn path.
//!
//! Verified flow (`konung.res`):
//! - `FUN_0043ADF0` rebuilds the active pool from `Game.N +0x21900` on map
//!   entry, filtered by location and the `+0x00/+0x04` timestamp pair.
//! - `FUN_00438CC8` resolves the ground sprite from the first item when the
//!   record has exactly one item and no money; otherwise `0xA3`.
//! - `FUN_00421B50` calls `FUN_004110EC` only after an interaction arrival
//!   (`Entity+0x03 & 0x40`, set by a right-click). A plain left-click move
//!   clears that bit and therefore never picks anything up.
//! - `FUN_004110EC` transfers money and then each item with the original
//!   weight/capacity checks.
//! - `FUN_0041093C` respawns a sole class-9 subtype-1/2 item: it allocates a
//!   new instance cloned from the collected one and stores `-frame` in the
//!   source timestamps. Every other emptied record is permanently removed.

use super::*;
use crate::world_objects::{WORLD_OBJECT_GENERIC_SPRITE, WORLD_OBJECT_ITEM_SLOTS};

impl PlayableApp {
    /// Advance the monotonic original-frame clock (60 ms frames). This is the
    /// `_DAT_0081D640` analogue used by WorldObject activation/respawn.
    pub(super) fn advance_world_frame(&mut self, dt: f64) {
        self.world_frame_accum += dt.max(0.0) * LOGIC_TICK_HZ;
        let whole = self.world_frame_accum.floor();
        if whole >= 1.0 {
            self.world_frame = self.world_frame.saturating_add(whole as u64);
            self.world_frame_accum -= whole;
            for _ in 0..whole as u64 {
                self.world.tick_prop_flames(self.current_map);
                // `FUN_0041D048` settlement branch: `(frame + 7) & 0xF == 0`,
                // i.e. once per 16 original frames for all 20 settlements.
                if self.world_frame % 16 == 9 {
                    self.tick_settlement_pass(1);
                }
            }
        }
    }

    /// Apply the original settlement target → prop variant linkage for one
    /// target (`FUN_004172AC`): `object_id = table[key] + state + 30`, then
    /// initialize the prop animation frame and its FLAMES emitter phases.
    pub(super) fn apply_settlement_prop(
        &mut self,
        loc: u32,
        target: &crate::population::SettlementTarget,
    ) {
        let Some(base) = self
            .world
            .res
            .item_db
            .as_ref()
            .and_then(|db| db.settlement_prop_base(target.key as usize))
        else {
            return;
        };
        let object_id = base as usize + target.state as usize + 30;
        if !(30..512).contains(&object_id) {
            return;
        }
        let frame_count = self.world.res.object_frame_count(object_id).max(1);
        let frame =
            (prop_variant_hash(loc, target.prop_index, object_id, 0) % frame_count as u32) as u8;
        // `FUN_004172AC` copies the object record's `+0x14` palette offset into
        // the prop record (`prop+0x04`).
        let palette = {
            let block = self.world.res.object_block(object_id);
            if block.len() >= 0x18 {
                crate::res::u32(block, 0x14)
            } else {
                0
            }
        };
        let emitters: Vec<crate::res::FlameEmitter> = self
            .world
            .res
            .object_flames(object_id)
            .into_iter()
            .filter(|e| crate::res::flame_range(e.selector).is_some())
            .collect();
        let mut frames = Vec::with_capacity(emitters.len());
        for (i, emitter) in emitters.iter().enumerate() {
            let (min, max) = crate::res::flame_range(emitter.selector).expect("filtered");
            let span = (max - min) as u32;
            frames.push(
                min + (prop_variant_hash(loc, target.prop_index, object_id, i as u32 + 1) % span)
                    as u8,
            );
        }
        let Some(loc_data) = self.world.loc_mut(loc) else {
            return;
        };
        if let Some(prop) = loc_data
            .props
            .iter_mut()
            .find(|p| p.ri == target.prop_index as usize)
        {
            prop.object_id = object_id;
            prop.frame = frame;
            prop.palette = palette;
            prop.flame_emitters = emitters;
            prop.flame_frames = frames;
        }
    }

    /// Apply the target → prop variant linkage (`FUN_0043ADF0` `0x43B7A7`) for
    /// every target of the current map (map load).
    pub(super) fn apply_settlement_props(&mut self) {
        let loc = self.current_map;
        let targets: Vec<crate::population::SettlementTarget> = self
            .settlements
            .iter()
            .filter(|s| s.loc_id as u32 == loc)
            .flat_map(|s| s.targets.iter().copied())
            .collect();
        for target in targets {
            self.apply_settlement_prop(loc, &target);
        }
    }

    /// Rebuild the active WorldObject list for the current map
    /// (`FUN_0043ADF0` `0x43BC05..0x43BD89`).
    pub(super) fn rebuild_world_objects(&mut self) {
        let loc = self.current_map;
        let now = self.world_frame as i64;
        let mut active = Vec::new();
        for source in &self.world_object_sources {
            if source.loc as u32 != loc || !source.active_at(now) {
                continue;
            }
            let items: Vec<u16> = source.items.iter().copied().filter(|&id| id != 0).collect();
            if items.is_empty() && source.money == 0 {
                continue;
            }
            let first = items.first().and_then(|&id| self.world.res.item_info(id));
            let sprite = world_object_sprite(first, &items, source.money);
            active.push(ActiveWorldObject {
                source: source.index,
                x: source.x as i32,
                y: source.y as i32,
                money: source.money,
                items,
                sprite,
            });
        }
        self.active_world_objects = active;
    }

    /// Diagnostic: runtime cell of active ground marker `index`.
    pub fn debug_active_world_object_cell(&self, index: usize) -> Option<(i32, i32)> {
        self.active_world_objects.get(index).map(|o| o.cell())
    }

    /// Diagnostic: number of active ground markers on the current map.
    pub fn debug_active_world_object_count(&self) -> usize {
        self.active_world_objects.len()
    }

    /// Run the original pickup/interaction handler for the protagonist when
    /// the arrival cell contains an active WorldObject.
    pub(super) fn try_pickup_here(&mut self) {
        let cell = (self.player_x.round() as i32, self.player_y.round() as i32);
        let Some(index) = self
            .active_world_objects
            .iter()
            .position(|o| o.cell() == cell)
        else {
            return;
        };

        // `FUN_00421B50`: a buried marker (negative `+0x10`) is only collected
        // when the leader carries the shovel — `FUN_00433A8C(0x19)` scans the
        // 32 inventory slots for class `0x0B` subtype `0x18`.
        if self.active_world_objects[index].money < 0 && !self.shovel_in_bag() {
            return;
        }

        // `0x41115B`: a WorldObject whose runtime `+0x0E` source index is 0
        // starts the victory movie instead of the ordinary pickup path.
        if self.active_world_objects[index].source == 0 {
            if crate::video::request(
                crate::video::VideoClip::Win,
                crate::video::VideoAfter::Menu { game_active: false },
            ) {
                self.game_over = true;
            }
            return;
        }

        // Money is transferred first (`0x411181..0x411190`) and cleared even if
        // the item transfer later fails. `FUN_0043EAA2` is `abs`, so a buried
        // spot's `-100` pays out `+100`.
        let money = (self.active_world_objects[index].money as i32).abs();
        if money != 0 {
            self.player.money = self.player.money.saturating_add(money);
        }
        if let Some(obj) = self.active_world_objects.get_mut(index) {
            obj.money = 0;
        }
        if self.active_world_objects[index].items.is_empty() {
            let source_index = self.active_world_objects[index].source;
            self.active_world_objects.remove(index);
            self.finish_world_object(source_index);
            return;
        }

        let threshold = crate::inventory::capacity(self.player.endurance_base);
        let mut weight = self.carried_weight();
        let mut transferred = 0usize;

        loop {
            let item = match self.active_world_objects[index].items.first().copied() {
                Some(id) if id != 0 => id,
                _ => break,
            };
            let item_weight = crate::inventory::item_weight(&self.world.res, item);
            // `0x4111f3..0x411216`: strict `threshold < weight` failure. The
            // comparison is signed (original `int`), so a negative-weight item
            // such as «Сын Луны» never trips it.
            if threshold < weight.saturating_add(item_weight) {
                self.set_pickup_message("Превышен вес");
                break;
            }
            // Class-12 ammo merges into an existing stack; everything else is
            // inserted at slot 0 and shifts the rest up (`0x41125b..0x41129e`).
            let stacked;
            {
                let PlayableApp { world, player, .. } = self;
                let mut slots = crate::inventory::SlotsMut {
                    equipment: &mut player.equipment,
                    inventory: &mut player.inventory,
                    additional: &mut player.additional_equipment,
                    special: &mut player.special_equipment,
                };
                stacked = crate::inventory::try_stack_ammo(&mut world.res, &mut slots, item);
            }
            if !stacked {
                // `0x4111a2`: 32 inventory slots, `0` = empty.
                if !self.player.inventory.contains(&0) {
                    self.set_pickup_message("Мешок у тебя не резиновый!");
                    break;
                }
                for i in (0..self.player.inventory.len() - 1).rev() {
                    self.player.inventory[i + 1] = self.player.inventory[i];
                }
                self.player.inventory[0] = item;
            }
            weight += item_weight;
            transferred += 1;
            // Pop the item from the runtime pile (`memmove +0x14..`).
            self.active_world_objects[index].items.remove(0);
            if self.active_world_objects[index].items.is_empty() {
                break;
            }
        }

        if transferred == 0 {
            return;
        }

        // Refresh the ground sprite after a partial transfer.
        if !self.active_world_objects[index].items.is_empty() {
            let first = self.active_world_objects[index].items.first().copied();
            let info = first.and_then(|id| self.world.res.item_info(id));
            let obj = &mut self.active_world_objects[index];
            obj.sprite = world_object_sprite(info, &obj.items, obj.money);
            return;
        }

        // Item list empty: remove the runtime marker and finalize the source.
        let source_index = self.active_world_objects[index].source;
        self.active_world_objects.remove(index);
        self.finish_world_object(source_index);
    }

    /// Spawn the original corpse marker (`FUN_00418480`) at the dead actor's
    /// cell. The record is a runtime `WorldObjectSource`, so the existing
    /// right-click pickup path consumes it like an authored pile.
    pub(super) fn spawn_corpse_loot(
        &mut self,
        entity: &crate::population::Entity,
        cell: (i32, i32),
        killer_level: u8,
        bonus: i32,
    ) {
        let plan = {
            let res = &self.world.res;
            let Some(db) = res.item_db.as_ref() else {
                return;
            };
            crate::combat::loot::plan_corpse_loot(
                db,
                &mut self.combat_rng,
                entity.stats.money,
                &entity.inventory,
                entity.stats.raw_0x52,
                killer_level,
                bonus,
                self.player.identification,
            )
        };
        if plan.is_empty() {
            return;
        }
        let mut ids = plan.existing;
        for spec in plan.generated {
            if ids.len() >= WORLD_OBJECT_ITEM_SLOTS {
                break;
            }
            if let Some(id) = self.world.res.alloc_item_instance(
                spec.class,
                spec.subtype,
                spec.condition,
                spec.modifier,
                spec.durability,
                spec.poison,
                spec.flags,
            ) {
                ids.push(id);
            }
        }
        let mut items = [0u16; WORLD_OBJECT_ITEM_SLOTS];
        for (i, id) in ids.iter().take(WORLD_OBJECT_ITEM_SLOTS).enumerate() {
            items[i] = *id;
        }
        let money = plan.money.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        if items.iter().all(|&id| id == 0) && money == 0 {
            return;
        }
        let x = cell.0.clamp(0, u16::MAX as i32) as u16;
        let y = cell.1.clamp(0, u16::MAX as i32) as u16;
        let loc = self.current_map as u16;
        // `FUN_00418480` merges into an existing marker on the same cell.
        if let Some(existing) = self
            .world_object_sources
            .iter_mut()
            .find(|s| s.loc == loc && s.x == x && s.y == y)
        {
            existing.money = existing.money.saturating_add(money);
            let mut slot = 0usize;
            for id in items.iter().copied().filter(|&id| id != 0) {
                while slot < WORLD_OBJECT_ITEM_SLOTS && existing.items[slot] != 0 {
                    slot += 1;
                }
                if slot >= WORLD_OBJECT_ITEM_SLOTS {
                    break;
                }
                existing.items[slot] = id;
            }
            self.rebuild_world_objects();
            return;
        }
        let index = self.world_object_sources.len();
        self.world_object_sources.push(WorldObjectSource {
            index,
            t0: 0,
            t4: 0,
            x,
            y,
            money,
            loc,
            items,
        });
        self.rebuild_world_objects();
    }

    /// Original `FUN_0041093C` source handling after the pile is emptied.
    fn finish_world_object(&mut self, source_index: usize) {
        let Some(source) = self.world_object_sources.get(source_index) else {
            return;
        };
        let item = source.items[0];
        let single = item != 0
            && source.items[1..WORLD_OBJECT_ITEM_SLOTS]
                .iter()
                .all(|&id| id == 0);
        let is_herb = single
            && self
                .world
                .res
                .item_info(item)
                .map(|i| i.class == 9 && matches!(i.subtype, 1 | 2))
                .unwrap_or(false);

        if is_herb {
            let now = self.world_frame as i64;
            match self.world.res.alloc_item_like(item) {
                Some(new_id) => {
                    if let Some(source) = self.world_object_sources.get_mut(source_index) {
                        source.items[0] = new_id;
                        source.deactivate_at(now);
                    }
                }
                None => {
                    // `FUN_00438884` failed: `FUN_0041093C` clears `+0x12`.
                    if let Some(source) = self.world_object_sources.get_mut(source_index) {
                        source.remove();
                    }
                }
            }
        } else if let Some(source) = self.world_object_sources.get_mut(source_index) {
            source.remove();
        }
    }

    pub(super) fn set_pickup_message(&mut self, message: &str) {
        self.pickup_message = Some(message.to_string());
        self.pickup_message_t = 3.0;
    }

    /// `FUN_00433A8C(0x19)`: the leader's 32 inventory slots contain a
    /// class-`0x0B` subtype-`0x18` («Лопата») instance. Only the bag counts —
    /// equipment/additional slots are not scanned by the original.
    fn shovel_in_bag(&self) -> bool {
        self.player.inventory.iter().copied().any(|id| {
            id != 0 && self.world.res.item_class_subtype(id) == Some((11, 24))
        })
    }

    /// `FUN_0041B7E4`: sum of every carried item's weight.
    fn carried_weight(&self) -> i32 {
        self.slots(crate::inventory::InventoryOwner::Protagonist)
            .map(|slots| crate::inventory::carried_weight(&self.world.res, &slots))
            .unwrap_or(0)
    }
}

/// `FUN_00438CC8`: a record with exactly one item and no money uses that
/// item's world sprite; anything else uses the generic loot icon.
fn world_object_sprite(first: Option<crate::item::ItemInfo>, items: &[u16], money: i16) -> u16 {
    if items.len() == 1 && money == 0 {
        first
            .map(|i| i.world_sprite)
            .unwrap_or(WORLD_OBJECT_GENERIC_SPRITE)
    } else {
        WORLD_OBJECT_GENERIC_SPRITE
    }
}

/// Deterministic visual-only hash for per-prop animation phases. The original
/// uses the game RNG at map load; using a stable hash keeps combat deterministic
/// while still varying the flame/prop start frames.
fn prop_variant_hash(loc: u32, prop_index: u16, object_id: usize, salt: u32) -> u32 {
    let mut h = 0x811C_9DC5u32;
    for value in [loc, prop_index as u32, object_id as u32, salt] {
        h ^= value;
        h = h.wrapping_mul(0x0100_0193);
        h ^= h >> 13;
    }
    h
}
