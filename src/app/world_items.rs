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
        }
    }

    /// Rebuild the active WorldObject list for the current map
    /// (`FUN_0043ADF0` `0x43BC05..0x43BD89`).
    pub(super) fn rebuild_world_objects(&mut self) {
        let loc = self.current_map;
        let now = self.world_frame as i64;
        let mut active = Vec::new();
        for source in &self.world_object_sources {
            if source.loc as u32 != loc || !source.active_at(now) || source.money < 0 {
                continue;
            }
            let items: Vec<u16> = source
                .items
                .iter()
                .copied()
                .filter(|&id| id != 0)
                .collect();
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

    /// Run the original pickup/interaction handler for the protagonist when
    /// the arrival cell contains an active WorldObject.
    pub(super) fn try_pickup_here(&mut self) {
        let cell = (
            self.player_x.round() as i32,
            self.player_y.round() as i32,
        );
        let Some(index) = self
            .active_world_objects
            .iter()
            .position(|o| o.cell() == cell)
        else {
            return;
        };

        // Money is transferred first (`0x411181..0x411190`) and cleared even if
        // the item transfer later fails.
        let money = self.active_world_objects[index].money.max(0) as i32;
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

        let threshold =
            10_000u32 + (self.player.endurance_base as u32 * 1000) / 3;
        let mut weight = self.carried_weight();
        let mut transferred = 0usize;

        loop {
            let item = match self.active_world_objects[index].items.first().copied() {
                Some(id) if id != 0 => id,
                _ => break,
            };
            // `0x4111a2`: 32 inventory slots, `0` = empty.
            if !self.player.inventory.contains(&0) {
                self.set_pickup_message("Мешок у тебя не резиновый!");
                break;
            }
            let item_weight = self
                .world
                .res
                .item_info(item)
                .map(|i| i.weight as u32)
                .unwrap_or(0);
            // `0x4111f3..0x411216`: strict `threshold < weight` failure.
            if threshold < weight.saturating_add(item_weight) {
                self.set_pickup_message("Превышен вес");
                break;
            }
            // Insert at slot 0 and shift the rest up (`0x41125b..0x41129e`).
            for i in (0..self.player.inventory.len() - 1).rev() {
                self.player.inventory[i + 1] = self.player.inventory[i];
            }
            self.player.inventory[0] = item;
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
            let first = self.active_world_objects[index]
                .items
                .first()
                .copied();
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

    fn set_pickup_message(&mut self, message: &str) {
        self.pickup_message = Some(message.to_string());
        self.pickup_message_t = 3.0;
    }

    /// `FUN_0041B7E4`: sum of every carried item's weight.
    fn carried_weight(&self) -> u32 {
        self.player
            .equipment
            .iter()
            .chain(self.player.additional_equipment.iter())
            .chain(self.player.inventory.iter())
            .chain(std::iter::once(&self.player.special_equipment))
            .filter(|&&id| id != 0)
            .filter_map(|&id| self.world.res.item_info(id))
            .map(|i| i.weight as u32)
            .sum()
    }
}

/// `FUN_00438CC8`: a record with exactly one item and no money uses that
/// item's world sprite; anything else uses the generic loot icon.
fn world_object_sprite(
    first: Option<crate::item::ItemInfo>,
    items: &[u16],
    money: i16,
) -> u16 {
    if items.len() == 1 && money == 0 {
        first
            .map(|i| i.world_sprite)
            .unwrap_or(WORLD_OBJECT_GENERIC_SPRITE)
    } else {
        WORLD_OBJECT_GENERIC_SPRITE
    }
}
