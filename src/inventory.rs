//! Runtime inventory: owner containers, weight, ammo stacking, crafting and
//! potion use.
//!
//! The original inventory lives in four fields per character/entity:
//!
//! ```text
//! Entity+0x66..+0x71  equipment[6]
//! Entity+0x72..+0xB1  inventory[32]
//! Entity+0xB2..+0xBB  additional[5]
//! Entity+0xCC         special
//! ```
//!
//! Item instances are the 16-byte `Game.N` records. Everything here mutates
//! only through [`Res`]'s instance helpers, so the decoded class/subtype,
//! quality (`+0x04`), quantity (`+0x04 u32` for class 12) and flags stay in one
//! place.
//!
//! Crafting is the verified player alchemy path (`FUN_0041DAB0` ->
//! `FUN_0041BB70`): a held class-9 item is combined with another class-9 item
//! using the 33-record table at VA `0x450DF4`. The held item becomes the result;
//! the target becomes a `out_b` item (`3` = empty jar, `0xFF` = consumed).
//!
//! Item use (`FUN_0041DAB0`) never mutates a character directly: it returns a
//! list of [`StatOp`] mutations that the caller applies to either a
//! [`PlayerCharacter`] or a companion [`Entity`] through
//! [`apply_player_ops`]/[`apply_entity_ops`]. The temporary-effect fields
//! `Entity+0xBC..+0xC2` (saved stats) and `+0xCA/+0xCE` (effect id / frame
//! timer) are restored by [`tick_player_temp`]/[`tick_entity_temp`] once the
//! timer reaches zero.

use crate::character::PlayerCharacter;
use crate::population::Entity;
use crate::res::Res;

/// Main equipment slots (`Entity+0x66..+0x71`, six `u16` item ids).
pub const MAIN_SLOTS: usize = 6;
/// Inventory slots (`Entity+0x72..+0xB1`, 32 `u16` item ids).
pub const INVENTORY_SLOTS: usize = 32;
/// Additional equipment slots (`Entity+0xB2..+0xBB`, five `u16` item ids).
pub const ADDITIONAL_SLOTS: usize = 5;

/// One editable inventory owner: the protagonist or a recruited companion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InventoryOwner {
    /// The protagonist, whose containers live in `PlayerCharacter`.
    Protagonist,
    /// Companion at this party-slot index (1-based entries follow slot 0).
    Party(usize),
}

/// A concrete item container inside one owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemLocation {
    /// Main equipment slot `0..MAIN_SLOTS` (`Entity+0x66` base).
    Main(usize),
    /// Additional equipment slot `0..ADDITIONAL_SLOTS` (`Entity+0xB2` base).
    Additional(usize),
    /// The single special/ammo slot (`Entity+0xCC`).
    Special,
    /// Inventory slot `0..INVENTORY_SLOTS` (`Entity+0x72` base).
    Inventory(usize),
}

/// The item currently on the mouse cursor (original `_81D670`/`_81D400`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeldItem {
    /// 16-byte `Game.N` item instance id (0 = nothing held).
    pub id: u16,
    /// Owner the item came from.
    pub owner: InventoryOwner,
    /// Container the item was lifted from.
    pub source: Option<ItemLocation>,
}

/// Immutable view of one owner's four containers.
#[derive(Clone, Copy, Debug)]
pub struct SlotsRef<'a> {
    /// Six main equipment ids (`Entity+0x66..+0x71`).
    pub equipment: &'a [u16; MAIN_SLOTS],
    /// 32 inventory ids (`Entity+0x72..+0xB1`).
    pub inventory: &'a [u16; INVENTORY_SLOTS],
    /// Five additional equipment ids (`Entity+0xB2..+0xBB`).
    pub additional: &'a [u16; ADDITIONAL_SLOTS],
    /// Single special/ammo id (`Entity+0xCC`).
    pub special: u16,
}

/// Mutable view of one owner's four containers.
pub struct SlotsMut<'a> {
    /// Six main equipment ids (`Entity+0x66..+0x71`).
    pub equipment: &'a mut [u16; MAIN_SLOTS],
    /// 32 inventory ids (`Entity+0x72..+0xB1`).
    pub inventory: &'a mut [u16; INVENTORY_SLOTS],
    /// Five additional equipment ids (`Entity+0xB2..+0xBB`).
    pub additional: &'a mut [u16; ADDITIONAL_SLOTS],
    /// Single special/ammo id (`Entity+0xCC`).
    pub special: &'a mut u16,
}

impl<'a> SlotsRef<'a> {
    /// Item id at one container slot; `None` when the slot index is out of range.
    ///
    /// `Special` always resolves to the single `+0xCC` word.
    pub fn get(&self, loc: ItemLocation) -> Option<u16> {
        match loc {
            ItemLocation::Main(i) => self.equipment.get(i).copied(),
            ItemLocation::Additional(i) => self.additional.get(i).copied(),
            ItemLocation::Special => Some(self.special),
            ItemLocation::Inventory(i) => self.inventory.get(i).copied(),
        }
    }
}

impl<'a> SlotsMut<'a> {
    /// Borrow the mutable view as a read-only [`SlotsRef`].
    pub fn as_ref(&self) -> SlotsRef<'_> {
        SlotsRef {
            equipment: &*self.equipment,
            inventory: &*self.inventory,
            additional: &*self.additional,
            special: *self.special,
        }
    }

    /// Item id at one container slot; `None` when the slot index is out of range.
    pub fn get(&self, loc: ItemLocation) -> Option<u16> {
        match loc {
            ItemLocation::Main(i) => self.equipment.get(i).copied(),
            ItemLocation::Additional(i) => self.additional.get(i).copied(),
            ItemLocation::Special => Some(*self.special),
            ItemLocation::Inventory(i) => self.inventory.get(i).copied(),
        }
    }

    /// Take the item out of the slot. Inventory removal shifts the following
    /// entries left (`FUN_0041E160`); equipment/special slots are simply
    /// cleared.
    pub fn take(&mut self, loc: ItemLocation) -> Option<u16> {
        if let ItemLocation::Inventory(i) = loc {
            if i >= INVENTORY_SLOTS {
                return None;
            }
            let id = self.inventory[i];
            for j in i..INVENTORY_SLOTS - 1 {
                self.inventory[j] = self.inventory[j + 1];
            }
            self.inventory[INVENTORY_SLOTS - 1] = 0;
            return Some(id);
        }
        let slot = match loc {
            ItemLocation::Main(i) => self.equipment.get_mut(i)?,
            ItemLocation::Additional(i) => self.additional.get_mut(i)?,
            ItemLocation::Special => self.special,
            ItemLocation::Inventory(_) => unreachable!(),
        };
        let id = *slot;
        *slot = 0;
        Some(id)
    }

    /// Write a slot; returns the displaced item id (if any).
    pub fn set(&mut self, loc: ItemLocation, id: u16) -> Option<u16> {
        let slot = match loc {
            ItemLocation::Main(i) => self.equipment.get_mut(i)?,
            ItemLocation::Additional(i) => self.additional.get_mut(i)?,
            ItemLocation::Special => self.special,
            ItemLocation::Inventory(i) => self.inventory.get_mut(i)?,
        };
        let old = *slot;
        *slot = id;
        if old == 0 {
            None
        } else {
            Some(old)
        }
    }

    /// First container holding `id`, in the original pick-up search order
    /// (`FUN_0041E8A8`: special, main, additional, inventory).
    pub fn find(&self, id: u16) -> Option<ItemLocation> {
        if id == 0 {
            return None;
        }
        if *self.special == id {
            return Some(ItemLocation::Special);
        }
        if let Some(i) = self.equipment.iter().position(|&v| v == id) {
            return Some(ItemLocation::Main(i));
        }
        if let Some(i) = self.additional.iter().position(|&v| v == id) {
            return Some(ItemLocation::Additional(i));
        }
        self.inventory
            .iter()
            .position(|&v| v == id)
            .map(ItemLocation::Inventory)
    }

    /// Remove the first occurrence of `id` from any container, in the
    /// [`Self::find`] order. Returns whether it was present.
    pub fn remove_id(&mut self, id: u16) -> bool {
        let Some(loc) = self.find(id) else {
            return false;
        };
        self.take(loc).is_some()
    }
}

/// Read-only owner snapshot used by weight, requirements and use/craft effects.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OwnerSnapshot {
    /// `Entity+0x33` character level.
    pub level: u8,
    /// `Entity+0x34` i32 money.
    pub money: i32,
    /// `Entity+0x38` u32 available/free XP.
    pub available_xp: u32,
    /// `Entity+0x3C` u32 invested XP.
    pub invested_xp: u32,
    /// `Entity+0x40` melee mastery.
    pub melee_mastery: u16,
    /// `Entity+0x42` trade skill.
    pub trade: u16,
    /// `Entity+0x44` archery mastery.
    pub archery_mastery: u16,
    /// `Entity+0x46` building skill.
    pub building: u16,
    /// `Entity+0x48` healing skill (drives alchemy quality and skill gain).
    pub healing: u16,
    /// `Entity+0x4A` identification skill.
    pub identification: u16,
    /// `Entity+0x4C` smithing skill.
    pub smithing: u16,
    /// `Entity+0x4E` innate armor rating (without item armor).
    pub base_armor: u16,
    /// `Entity+0x50` periodic (poison) damage per 16-frame pass.
    pub periodic_damage: u16,
    /// `Entity+0x52` venom value copied into corpse-loot stingers; `0` for the
    /// protagonist save block, which carries no such field.
    pub raw_0x52: u16,
    /// `Entity+0x12` raw byte: active weapon slot, also read by the character
    /// screen's overlapping-slot (storage 0/2) branch.
    pub raw_0x12: u8,
    /// `Entity+0x03` flags (`0x04` battle stance, `0x80` dead).
    pub flags_b: u8,
    /// Whether the owner is dead (`flags_b & 0x80`).
    pub dead: bool,
    /// `Entity+0x54` dexterity base.
    pub dexterity_base: u16,
    /// `Entity+0x56` dexterity current (temporary effects lower it).
    pub dexterity_current: u16,
    /// `Entity+0x58` charisma base.
    pub charisma_base: u16,
    /// `Entity+0x5A` charisma current.
    pub charisma_current: u16,
    /// `Entity+0x5E` strength base.
    pub strength_base: u16,
    /// `Entity+0x60` strength current.
    pub strength_current: u16,
    /// `Entity+0x62` endurance base (capacity = `*1000/3 + 10000`).
    pub endurance_base: u16,
    /// `Entity+0x64` endurance current.
    pub endurance_current: u16,
    /// `Entity+0x5C` HP in the internal 16x scale (1600 = full).
    pub hp_internal: u16,
    /// `Entity+0xCE` temporary-effect timer in logic frames (0 = none).
    pub temp_effect_timer: u16,
    /// `Entity+0xCA` temporary-effect id.
    pub temp_effect_id: u16,
    /// `Entity+0xBC` saved dexterity current (temporary-effect backup).
    pub saved_dexterity: u16,
    /// `Entity+0xBE` saved charisma current.
    pub saved_charisma: u16,
    /// `Entity+0xC0` saved strength current.
    pub saved_strength: u16,
    /// `Entity+0xC2` saved endurance current.
    pub saved_endurance: u16,
}

impl OwnerSnapshot {
    /// Snapshot the protagonist's live `PlayerCharacter` fields.
    ///
    /// The player record has no `Entity+0x52`, so `raw_0x52` is reported as `0`.
    pub fn from_player(p: &PlayerCharacter) -> OwnerSnapshot {
        OwnerSnapshot {
            level: p.level,
            money: p.money,
            available_xp: p.available_xp,
            invested_xp: p.invested_xp,
            melee_mastery: p.melee_mastery,
            trade: p.trade,
            archery_mastery: p.archery_mastery,
            building: p.building,
            healing: p.healing,
            identification: p.identification,
            smithing: p.smithing,
            base_armor: p.base_armor,
            periodic_damage: p.periodic_damage,
            raw_0x52: 0,
            raw_0x12: p.raw_0x12,
            flags_b: p.flags_b,
            dead: p.dead,
            dexterity_base: p.dexterity_base,
            dexterity_current: p.dexterity_current,
            charisma_base: p.charisma_base,
            charisma_current: p.charisma_current,
            strength_base: p.strength_base,
            strength_current: p.strength_current,
            endurance_base: p.endurance_base,
            endurance_current: p.endurance_current,
            hp_internal: p.hp_internal,
            temp_effect_timer: p.temp_effect_timer,
            temp_effect_id: p.temp_effect_id,
            saved_dexterity: p.saved_dexterity,
            saved_charisma: p.saved_charisma,
            saved_strength: p.saved_strength,
            saved_endurance: p.saved_endurance,
        }
    }

    /// Snapshot a companion's authored `Game.N` entity record.
    ///
    /// `dead` is derived from `flags_b & 0x80`; every other field is a verbatim
    /// `Entity+0x..` read.
    pub fn from_entity(e: &Entity) -> OwnerSnapshot {
        let s = &e.stats;
        OwnerSnapshot {
            level: s.level,
            money: s.money,
            available_xp: s.available_xp,
            invested_xp: s.invested_xp,
            melee_mastery: s.melee_mastery,
            trade: s.trade,
            archery_mastery: s.archery_mastery,
            building: s.building,
            healing: s.healing,
            identification: s.identification,
            smithing: s.smithing,
            base_armor: s.base_armor,
            periodic_damage: s.periodic_damage,
            raw_0x52: s.raw_0x52,
            raw_0x12: e.raw_0x12,
            flags_b: e.flags_b,
            dead: e.flags_b & 0x80 != 0,
            dexterity_base: s.dexterity_base,
            dexterity_current: s.dexterity_current,
            charisma_base: s.charisma_base,
            charisma_current: s.charisma_current,
            strength_base: s.strength_base,
            strength_current: s.strength_current,
            endurance_base: s.endurance_base,
            endurance_current: s.endurance_current,
            hp_internal: s.hp_internal,
            temp_effect_timer: s.temp_effect_timer,
            temp_effect_id: s.temp_effect_id,
            saved_dexterity: s.saved_dexterity,
            saved_charisma: s.saved_charisma,
            saved_strength: s.saved_strength,
            saved_endurance: s.saved_endurance,
        }
    }
}

/// Weight of one item instance. Class 12 (`arrows`) uses `per-unit * quantity`.
///
/// Signed: the original (`FUN_0041B0C4`) returns the sign-extended `i16`
/// template weight, so «Сын Луны» contributes −10000.
pub fn item_weight(res: &Res, id: u16) -> i32 {
    if id == 0 {
        return 0;
    }
    let Some(info) = res.item_info(id) else {
        return 0;
    };
    let class = res.item_class(id).unwrap_or(info.class);
    if class == 12 {
        let q = res.item_quantity(id).unwrap_or(0);
        info.weight.saturating_mul(q as i32)
    } else {
        info.weight
    }
}

/// `FUN_0041B7E4`: sum of every carried instance (main, additional, inventory,
/// special).
pub fn carried_weight(res: &Res, slots: &SlotsRef) -> i32 {
    slots
        .equipment
        .iter()
        .chain(slots.additional.iter())
        .chain(slots.inventory.iter())
        .chain(std::iter::once(&slots.special))
        .copied()
        .map(|id| item_weight(res, id))
        .sum()
}

/// `(endurance_base * 1000) / 3 + 10000`.
pub fn capacity(endurance_base: u16) -> i32 {
    10_000 + (endurance_base as i32 * 1000) / 3
}

/// First free inventory slot.
pub fn free_inventory_slot(inv: &[u16; INVENTORY_SLOTS]) -> Option<usize> {
    inv.iter().position(|&id| id == 0)
}

/// `FUN_0042462C`: insert at inventory slot 0 and shift. Fails when full.
pub fn insert_inventory_front(inv: &mut [u16; INVENTORY_SLOTS], id: u16) -> bool {
    if free_inventory_slot(inv).is_none() {
        return false;
    }
    for i in (0..INVENTORY_SLOTS - 1).rev() {
        inv[i + 1] = inv[i];
    }
    inv[0] = id;
    true
}

/// Remove the first inventory occurrence of `id`, shifting the rest down.
pub fn remove_inventory_id(inv: &mut [u16; INVENTORY_SLOTS], id: u16) -> bool {
    let Some(pos) = inv.iter().position(|&v| v == id) else {
        return false;
    };
    for i in pos..INVENTORY_SLOTS - 1 {
        inv[i] = inv[i + 1];
    }
    inv[INVENTORY_SLOTS - 1] = 0;
    true
}

/// Class gating for the six main equipment slots (`FUN_0041E450`): the item
/// class is the slot index (`0..=5`).
pub fn main_slot_accepts(class: u8, slot: usize) -> bool {
    slot < MAIN_SLOTS && class as usize == slot
}

/// Class gating for the five additional slots (`FUN_0041E6BC`).
pub fn additional_slot_for_class(class: u8, code: u16) -> Option<usize> {
    let slot = code.checked_sub(0x0E)? as usize;
    if slot >= ADDITIONAL_SLOTS {
        return None;
    }
    match (class, slot) {
        (6, 0) => Some(0),
        (7, 1) | (7, 2) => Some(slot),
        (8, 3) | (8, 4) => Some(slot),
        _ => None,
    }
}

/// `FUN_0041B1A4`: requirement stat id -> current value.
pub fn requirement_stat_value(
    snap: &OwnerSnapshot,
    res: &Res,
    slots: &SlotsRef,
    stat_id: u16,
) -> i32 {
    match stat_id {
        1 | 3 => snap.dexterity_current as i32,
        2 => snap.raw_0x52 as i32,
        4 => snap.charisma_current as i32 / 16,
        5 => snap.hp_internal as i32,
        6 => snap.strength_current as i32,
        7 => {
            let mut armor = snap.base_armor as i32;
            for slot in 3..6 {
                if let Some(info) = res.item_info(slots.equipment[slot]) {
                    armor += info.rating as i32;
                }
            }
            armor
        }
        8 => {
            let s = snap.raw_0x12 as usize;
            if s < MAIN_SLOTS {
                res.item_info(slots.equipment[s])
                    .map(|i| i.rating as i32)
                    .unwrap_or(0)
            } else {
                0
            }
        }
        _ => 0,
    }
}

/// `FUN_004182D4`: prototype requirement check. The condition-modifier table
/// requirement branch is not applied (documented unknown layout); the
/// prototype requirement (`prototype+0x30/+0x32`) is.
pub fn requirement_ok(snap: &OwnerSnapshot, res: &Res, slots: &SlotsRef, id: u16) -> bool {
    let Some(info) = res.item_info(id) else {
        return false;
    };
    if info.requirement_value == 0 {
        return true;
    }
    requirement_stat_value(snap, res, slots, info.requirement_stat) >= info.requirement_value as i32
}

/// `FUN_004104D4`: auto-equip one **empty** main slot from the inventory.
///
/// The original scans the 32 inventory slots in order and takes the first item
/// whose class byte (`ItemInstance+0x00`) equals the slot index and that passes
/// the requirement predicate (`FUN_004182D4`); it then writes the slot and
/// consumes the inventory entry (`FUN_00421AD0`, shift-down + clear the tail).
/// An already occupied slot reports success without scanning.
pub fn auto_equip_slot(snap: &OwnerSnapshot, res: &Res, slots: &mut SlotsMut, slot: usize) -> bool {
    if slot >= MAIN_SLOTS {
        return false;
    }
    if slots.equipment[slot] != 0 {
        return true;
    }
    for i in 0..INVENTORY_SLOTS {
        let id = slots.inventory[i];
        if id == 0 {
            continue;
        }
        let Some((class, _)) = res.item_class_subtype(id) else {
            continue;
        };
        if !main_slot_accepts(class, slot) {
            continue;
        }
        if !requirement_ok(snap, res, &slots.as_ref(), id) {
            continue;
        }
        slots.equipment[slot] = id;
        remove_inventory_id(slots.inventory, id);
        return true;
    }
    false
}

/// Result of [`refresh_equipment`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EquipmentRefresh {
    /// New `Entity+0x12` active weapon slot (`1` bow, `0` axe/slot 0, `2` rest).
    pub weapon_slot: u8,
    /// Whether the ammo slot, a main slot or the active weapon slot changed.
    pub changed: bool,
}

/// `FUN_004105A0`: NPC equipment refresh.
///
/// 1. If the ammo slot (`Entity+0xCC`) is empty, take the first class-12 item
///    from the inventory that passes the requirement check.
/// 2. Auto-equip slots in the original order `0, 1, 3, 4, 5, 2`.
/// 3. Recompute `Entity+0x12`: bow when ammo **and** slot 1 are present, else
///    slot 0 when it is filled, else `2`.
///
/// The original also bumps `_DAT_0081D5F0`; the remake has no such revision
/// counter (nothing caches a derived equipment value), so `changed` is exposed
/// for callers/tests instead.
pub fn refresh_equipment(
    snap: &OwnerSnapshot,
    res: &Res,
    slots: &mut SlotsMut,
) -> EquipmentRefresh {
    let mut changed = false;

    if *slots.special == 0 {
        for i in 0..INVENTORY_SLOTS {
            let id = slots.inventory[i];
            if id == 0 {
                continue;
            }
            if !matches!(res.item_class_subtype(id), Some((12, _))) {
                continue;
            }
            if !requirement_ok(snap, res, &slots.as_ref(), id) {
                continue;
            }
            *slots.special = id;
            remove_inventory_id(slots.inventory, id);
            changed = true;
            break;
        }
    }

    // Exact original order: `FUN_004104D4(0)`, `(1)`, `(3)`, `(4)`, `(5)`, `(2)`.
    for slot in [0usize, 1, 3, 4, 5, 2] {
        if slots.equipment[slot] == 0 && auto_equip_slot(snap, res, slots, slot) {
            changed = true;
        }
    }

    let weapon_slot = if *slots.special != 0 && slots.equipment[1] != 0 {
        1
    } else if slots.equipment[0] != 0 {
        0
    } else {
        2
    };
    EquipmentRefresh {
        weapon_slot,
        changed,
    }
}

/// `FUN_0041BB70` recipe record (table VA `0x450DF4`, 33 x 6 bytes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Recipe {
    /// Held item's class-9 subtype (first ingredient).
    pub a: u8,
    /// Target item's class-9 subtype (second ingredient).
    pub b: u8,
    /// Subtype written into the held item (the product).
    pub out_a: u8,
    /// Subtype written into the target (`3` empty jar, `0xFF` consumed).
    pub out_b: u8,
    /// Healing-skill gain, capped at 500 by [`craft`].
    pub skill_gain: u8,
    /// Quality formula selector: `0` skill only, `1` held quality + skill,
    /// `2` held quality + target quality, `3` `min(held, target)`,
    /// `4` target quality + skill.
    pub op: u8,
}

/// The 33-record alchemy table at VA `0x450DF4`, in file order.
pub const RECIPES: [Recipe; 33] = [
    Recipe {
        a: 3,
        b: 0,
        out_a: 6,
        out_b: 0xFF,
        skill_gain: 3,
        op: 0,
    },
    Recipe {
        a: 6,
        b: 0,
        out_a: 6,
        out_b: 0xFF,
        skill_gain: 3,
        op: 1,
    },
    Recipe {
        a: 6,
        b: 6,
        out_a: 6,
        out_b: 0xFF,
        skill_gain: 3,
        op: 2,
    },
    Recipe {
        a: 3,
        b: 1,
        out_a: 7,
        out_b: 0xFF,
        skill_gain: 2,
        op: 0,
    },
    Recipe {
        a: 7,
        b: 1,
        out_a: 7,
        out_b: 0xFF,
        skill_gain: 2,
        op: 1,
    },
    Recipe {
        a: 7,
        b: 7,
        out_a: 7,
        out_b: 0xFF,
        skill_gain: 2,
        op: 2,
    },
    Recipe {
        a: 3,
        b: 2,
        out_a: 5,
        out_b: 0xFF,
        skill_gain: 0,
        op: 0,
    },
    Recipe {
        a: 5,
        b: 2,
        out_a: 5,
        out_b: 0xFF,
        skill_gain: 0,
        op: 1,
    },
    Recipe {
        a: 5,
        b: 5,
        out_a: 5,
        out_b: 0xFF,
        skill_gain: 1,
        op: 2,
    },
    Recipe {
        a: 5,
        b: 6,
        out_a: 8,
        out_b: 3,
        skill_gain: 3,
        op: 3,
    },
    Recipe {
        a: 8,
        b: 8,
        out_a: 8,
        out_b: 0xFF,
        skill_gain: 3,
        op: 2,
    },
    Recipe {
        a: 5,
        b: 7,
        out_a: 9,
        out_b: 3,
        skill_gain: 3,
        op: 3,
    },
    Recipe {
        a: 9,
        b: 9,
        out_a: 9,
        out_b: 0xFF,
        skill_gain: 3,
        op: 2,
    },
    Recipe {
        a: 5,
        b: 9,
        out_a: 10,
        out_b: 3,
        skill_gain: 0,
        op: 3,
    },
    Recipe {
        a: 10,
        b: 10,
        out_a: 10,
        out_b: 0xFF,
        skill_gain: 3,
        op: 2,
    },
    Recipe {
        a: 6,
        b: 7,
        out_a: 11,
        out_b: 3,
        skill_gain: 3,
        op: 3,
    },
    Recipe {
        a: 11,
        b: 11,
        out_a: 11,
        out_b: 0xFF,
        skill_gain: 3,
        op: 2,
    },
    Recipe {
        a: 6,
        b: 9,
        out_a: 12,
        out_b: 3,
        skill_gain: 4,
        op: 3,
    },
    Recipe {
        a: 12,
        b: 12,
        out_a: 12,
        out_b: 0xFF,
        skill_gain: 4,
        op: 2,
    },
    Recipe {
        a: 7,
        b: 8,
        out_a: 13,
        out_b: 3,
        skill_gain: 4,
        op: 3,
    },
    Recipe {
        a: 13,
        b: 13,
        out_a: 13,
        out_b: 0xFF,
        skill_gain: 4,
        op: 2,
    },
    Recipe {
        a: 0,
        b: 3,
        out_a: 6,
        out_b: 0xFF,
        skill_gain: 3,
        op: 0,
    },
    Recipe {
        a: 0,
        b: 6,
        out_a: 6,
        out_b: 0xFF,
        skill_gain: 3,
        op: 4,
    },
    Recipe {
        a: 1,
        b: 3,
        out_a: 7,
        out_b: 0xFF,
        skill_gain: 2,
        op: 0,
    },
    Recipe {
        a: 1,
        b: 7,
        out_a: 7,
        out_b: 0xFF,
        skill_gain: 2,
        op: 4,
    },
    Recipe {
        a: 2,
        b: 3,
        out_a: 5,
        out_b: 0xFF,
        skill_gain: 0,
        op: 0,
    },
    Recipe {
        a: 2,
        b: 5,
        out_a: 5,
        out_b: 0xFF,
        skill_gain: 0,
        op: 4,
    },
    Recipe {
        a: 6,
        b: 5,
        out_a: 8,
        out_b: 3,
        skill_gain: 3,
        op: 3,
    },
    Recipe {
        a: 7,
        b: 5,
        out_a: 9,
        out_b: 3,
        skill_gain: 3,
        op: 3,
    },
    Recipe {
        a: 9,
        b: 5,
        out_a: 9,
        out_b: 3,
        skill_gain: 0,
        op: 3,
    },
    Recipe {
        a: 7,
        b: 6,
        out_a: 11,
        out_b: 3,
        skill_gain: 3,
        op: 3,
    },
    Recipe {
        a: 9,
        b: 6,
        out_a: 12,
        out_b: 3,
        skill_gain: 4,
        op: 3,
    },
    Recipe {
        a: 8,
        b: 7,
        out_a: 13,
        out_b: 3,
        skill_gain: 4,
        op: 3,
    },
];

/// Result of one alchemy merge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CraftResult {
    /// Item that becomes the product and remains on the cursor.
    pub result: u16,
    /// Target item returned as an empty jar / consumed instance.
    pub container: u16,
    /// Healing-skill gain (already applied to the caller's field by `craft`).
    pub skill_gain: u16,
    /// No table row matched; the original produces «Непонятная смесь».
    pub default_mix: bool,
}

/// `FUN_0041BB70`: combine held class-9 `a` with class-9 `b`. The held item
/// becomes the product; the target becomes `out_b` (`3` empty jar or `0xFF`
/// consumed). Returns `None` when the items are not two distinct class-9 items.
pub fn craft(res: &mut Res, healing: &mut u16, held: u16, target: u16) -> Option<CraftResult> {
    if held == 0 || target == 0 || held == target {
        return None;
    }
    let (ca, sa) = res.item_class_subtype(held)?;
    let (cb, sb) = res.item_class_subtype(target)?;
    if ca != 9 || cb != 9 {
        return None;
    }
    let quality = res.item_quality(held).unwrap_or(0.0);
    let target_quality = res.item_quality(target).unwrap_or(0.0);
    let recipe = RECIPES.iter().find(|r| r.a == sa && r.b == sb);
    let Some(recipe) = recipe else {
        res.set_item_subtype(held, 4);
        res.set_item_class(target, 0xFF);
        return Some(CraftResult {
            result: held,
            container: target,
            skill_gain: 0,
            default_mix: true,
        });
    };
    let skill = *healing as f32 * 0.01;
    let mut q = match recipe.op {
        0 => skill,
        1 => quality + skill,
        2 => quality + target_quality,
        3 => quality.min(target_quality),
        4 => target_quality + skill,
        _ => quality,
    };
    q = q.min(15.0).max(0.0);
    res.set_item_quality(held, q);
    res.set_item_subtype(held, recipe.out_a);
    res.set_item_subtype(target, recipe.out_b);
    if recipe.out_b == 0xFF {
        res.set_item_class(target, 0xFF);
    }
    let gain = recipe.skill_gain as u16;
    *healing = (*healing).saturating_add(gain).min(500);
    Some(CraftResult {
        result: held,
        container: target,
        skill_gain: gain,
        default_mix: false,
    })
}

/// One mutation produced by potion use.
///
/// The list is applied by [`apply_player_ops`] / [`apply_entity_ops`]; skill
/// operations are applied in a second pass after the common stat pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatOp {
    /// Halve current HP, leaving at least 1 internal point (`subtype 4`).
    HalveHp,
    /// Set HP absolutely, clamped to 1600.
    SetHp(u16),
    /// Add to HP, clamped to `0..=1600`.
    AddHp(i32),
    /// Write the periodic (poison) damage field.
    SetPeriodic(u16),
    /// Add to the periodic damage field, clamping at 0.
    AddPeriodic(i32),
    /// Write dexterity current.
    SetDexterityCurrent(u16),
    /// Add to dexterity current, clamping at 0.
    AddDexterityCurrent(i32),
    /// Add to charisma current, clamping at 0.
    AddCharismaCurrent(i32),
    /// Add to strength current, clamping at 1.
    AddStrengthCurrent(i32),
    /// Add to endurance current, clamping at 0.
    AddEnduranceCurrent(i32),
    /// Write the temporary-effect timer (`Entity+0xCE`).
    SetTempTimer(u16),
    /// Write the temporary-effect id (`Entity+0xCA`).
    SetTempId(u16),
    /// Copy the four current primary stats into `Entity+0xBC..+0xC2`.
    SaveStats,
    /// Add available XP (saturating).
    AddAvailableXp(u32),
    /// Add healing skill, capped at 500.
    AddHealing(u16),
    /// Add building skill.
    AddBuilding(u16),
    /// Add smithing skill.
    AddSmithing(u16),
    /// Add trade skill, capped at 100.
    AddTrade(u16),
    /// Add identification skill, capped at 100.
    AddIdentification(u16),
    /// Clear the dead flag (`flags_b & 0x80`).
    Revive,
}

/// Errors from using an item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UseError {
    /// The instance is missing, not class 9, or the subtype has no use case.
    NotUsable,
    /// The item needs a target item (subtype 7, oil).
    NeedsTarget,
    /// A temporary effect is already active (`Entity+0xCE != 0`).
    Blocked,
    /// Quality below the minimum the subtype needs (subtype 12 needs `>= 6`).
    TooWeak,
}

/// `FUN_0041DAB0` target-item use: a held class-9 item applied to another item.
///
/// The original dispatcher calls this variant only when the target item class
/// is not 9 (class 9 -> class 9 is the alchemy craft path):
/// - subtype 7 (`Масло`): target must be class 12 (arrows); sets `+0x02 = 0`
///   and consumes the oil into an empty jar;
/// - subtype 6 (`Яд`): target class 0/2/12 (weapon/arrow); writes `+0x0C`;
/// - every other subtype ignores the target and behaves like [`use_item`].
///
/// Any failed precondition returns `Err` **without** consuming the used item,
/// matching the original early `return 0`.
pub fn use_item_on(
    res: &mut Res,
    snapshot: &OwnerSnapshot,
    used_id: u16,
    target_id: u16,
) -> Result<Vec<StatOp>, UseError> {
    if used_id == 0 || target_id == 0 || used_id == target_id {
        return Err(UseError::NotUsable);
    }
    let (used_class, subtype) = res.item_class_subtype(used_id).ok_or(UseError::NotUsable)?;
    if used_class != 9 {
        return Err(UseError::NotUsable);
    }
    let (target_class, _) = res
        .item_class_subtype(target_id)
        .ok_or(UseError::NotUsable)?;
    if target_class == 9 {
        // Target class 9 belongs to the alchemy craft path.
        return Err(UseError::NotUsable);
    }
    let quality = res.item_quality(used_id).unwrap_or(0.0);
    match subtype {
        7 => {
            if target_class != 12 || quality < 0.0 {
                return Err(UseError::NotUsable);
            }
            if !res.set_item_modifier(target_id, 0) {
                return Err(UseError::NotUsable);
            }
            res.set_item_subtype(used_id, 3);
            Ok(Vec::new())
        }
        6 => {
            if !matches!(target_class, 0 | 2 | 12) {
                return Err(UseError::NotUsable);
            }
            let poison = quality.round().clamp(0.0, u16::MAX as f32) as u16;
            if !res.set_item_poison(target_id, poison) {
                return Err(UseError::NotUsable);
            }
            res.set_item_subtype(used_id, 3);
            Ok(Vec::new())
        }
        _ => use_item(res, snapshot, used_id),
    }
}

/// `FUN_0041DAB0` right-click use of a class-9 item. The item becomes an empty
/// jar (`subtype 3`) in place. `snapshot` must be taken before applying the
/// returned operations.
pub fn use_item(res: &mut Res, snapshot: &OwnerSnapshot, id: u16) -> Result<Vec<StatOp>, UseError> {
    let (class, subtype) = res.item_class_subtype(id).ok_or(UseError::NotUsable)?;
    if class != 9 {
        return Err(UseError::NotUsable);
    }
    let q = res.item_quality(id).unwrap_or(0.0);
    let qr = q.round();
    let ops = match subtype {
        4 => vec![StatOp::HalveHp],
        5 => vec![StatOp::SetHp(
            (q * 160.0 + snapshot.hp_internal as f32)
                .round()
                .clamp(0.0, 1600.0) as u16,
        )],
        6 => vec![StatOp::AddPeriodic(qr as i32)],
        7 => return Err(UseError::NeedsTarget),
        8 => {
            let v = (q * 10.0 - snapshot.base_armor as f32).round().max(0.0);
            vec![StatOp::SetPeriodic(v as u16)]
        }
        9 => {
            if snapshot.temp_effect_timer != 0 {
                return Err(UseError::Blocked);
            }
            let dex_third = (snapshot.dexterity_current / 3) as i32;
            let local = if (q as i32) < dex_third {
                qr as i32
            } else {
                dex_third
            };
            vec![
                StatOp::SaveStats,
                StatOp::SetTempTimer((local.max(0) as u16).saturating_mul(30)),
                StatOp::SetTempId(0),
                StatOp::AddDexterityCurrent(-(local.max(0) * 3)),
                StatOp::AddCharismaCurrent(local),
                StatOp::AddStrengthCurrent(local),
                StatOp::AddEnduranceCurrent(local),
            ]
        }
        10 => {
            if snapshot.temp_effect_timer != 0 {
                return Err(UseError::Blocked);
            }
            let local = qr as i32;
            vec![
                StatOp::SaveStats,
                StatOp::SetTempTimer((local.max(0) as u16).saturating_mul(30)),
                StatOp::SetTempId(0),
                StatOp::AddDexterityCurrent(local),
                StatOp::AddStrengthCurrent(local),
                StatOp::AddEnduranceCurrent(local),
                StatOp::SetHp(
                    (q * 160.0 + snapshot.hp_internal as f32)
                        .round()
                        .clamp(0.0, 1600.0) as u16,
                ),
            ]
        }
        11 => {
            let local = qr as i32;
            let mut ops = vec![
                StatOp::SetTempTimer((local.max(0) as u16).saturating_mul(30)),
                StatOp::SetTempId((local.max(0) as f32 * 0.1).round() as u16),
            ];
            if snapshot.temp_effect_timer == 0 {
                ops.insert(0, StatOp::SaveStats);
            }
            ops
        }
        12 => {
            let local = qr as i32;
            if local < 6 {
                return Err(UseError::TooWeak);
            }
            let mut ops = Vec::new();
            if snapshot.temp_effect_timer == 0 {
                ops.push(StatOp::SaveStats);
            }
            ops.push(StatOp::AddAvailableXp(((local - 5) * 100).max(0) as u32));
            ops.push(StatOp::SetTempTimer(
                (local.max(0) as u16).saturating_mul(60),
            ));
            ops.push(StatOp::SetTempId(0));
            ops.push(StatOp::AddStrengthCurrent(-(local.max(0) * 5)));
            ops.push(StatOp::AddHp(-(local.max(0) * 160)));
            if snapshot.charisma_current < 160 {
                ops.push(StatOp::SetHp(160));
            }
            ops
        }
        13 => {
            let mut ops = vec![StatOp::SetHp(1600)];
            if snapshot.dead {
                ops.push(StatOp::Revive);
            }
            ops
        }
        _ => return Err(UseError::NotUsable),
    };
    res.set_item_subtype(id, 3);
    Ok(ops)
}

/// Clamp `v` into `lo..=hi`.
fn clamp_i32(v: i32, lo: i32, hi: i32) -> i32 {
    v.max(lo).min(hi)
}

/// Apply the non-skill [`StatOp`] variants to raw stat fields.
///
/// HP is clamped to `0..=1600`; periodic/dexterity/charisma/endurance clamp at
/// 0, strength at 1; [`StatOp::SaveStats`] writes the four-current-stat tuple.
/// Skill operations are skipped here and applied by [`apply_skill_ops`].
fn apply_ops_common(
    hp: &mut u16,
    periodic: &mut u16,
    dex: &mut u16,
    cha: &mut u16,
    str_: &mut u16,
    end: &mut u16,
    available_xp: &mut u32,
    timer: &mut u16,
    effect: &mut u16,
    saved: &mut (u16, u16, u16, u16),
    dead: &mut bool,
    ops: &[StatOp],
) {
    for op in ops {
        match *op {
            StatOp::HalveHp => *hp = (*hp / 2).max(1),
            StatOp::SetHp(v) => *hp = v.min(1600),
            StatOp::AddHp(d) => *hp = clamp_i32(*hp as i32 + d, 0, 1600) as u16,
            StatOp::SetPeriodic(v) => *periodic = v,
            StatOp::AddPeriodic(d) => {
                *periodic = (*periodic as i32 + d).max(0).min(u16::MAX as i32) as u16
            }
            StatOp::SetDexterityCurrent(v) => *dex = v,
            StatOp::AddDexterityCurrent(d) => {
                *dex = (*dex as i32 + d).max(0).min(u16::MAX as i32) as u16
            }
            StatOp::AddCharismaCurrent(d) => {
                *cha = (*cha as i32 + d).max(0).min(u16::MAX as i32) as u16
            }
            StatOp::AddStrengthCurrent(d) => {
                *str_ = (*str_ as i32 + d).max(1).min(u16::MAX as i32) as u16
            }
            StatOp::AddEnduranceCurrent(d) => {
                *end = (*end as i32 + d).max(0).min(u16::MAX as i32) as u16
            }
            StatOp::SetTempTimer(v) => *timer = v,
            StatOp::SetTempId(v) => *effect = v,
            StatOp::SaveStats => {
                *saved = (*dex, *cha, *str_, *end);
            }
            StatOp::AddAvailableXp(v) => *available_xp = (*available_xp).saturating_add(v),
            StatOp::Revive => *dead = false,
            // Skill operations are applied by `apply_player_ops` /
            // `apply_entity_ops` after this common pass.
            StatOp::AddHealing(_)
            | StatOp::AddBuilding(_)
            | StatOp::AddSmithing(_)
            | StatOp::AddTrade(_)
            | StatOp::AddIdentification(_) => {}
        }
    }
}

/// Apply the five skill [`StatOp`] variants, each with its original cap
/// (healing 500, trade 100, identification 100; building/smithing uncapped).
fn apply_skill_ops(
    healing: &mut u16,
    building: &mut u16,
    smithing: &mut u16,
    trade: &mut u16,
    identification: &mut u16,
    ops: &[StatOp],
) {
    for op in ops {
        match *op {
            StatOp::AddHealing(v) => *healing = healing.saturating_add(v).min(500),
            StatOp::AddBuilding(v) => *building = building.saturating_add(v),
            StatOp::AddSmithing(v) => *smithing = smithing.saturating_add(v),
            StatOp::AddTrade(v) => *trade = trade.saturating_add(v).min(100),
            StatOp::AddIdentification(v) => {
                *identification = identification.saturating_add(v).min(100)
            }
            _ => {}
        }
    }
}

/// Apply potion-use operations to the protagonist.
///
/// The saved-stat backup is read back into the `PlayerCharacter` after the
/// common pass, and skill operations run in the second pass.
pub fn apply_player_ops(p: &mut PlayerCharacter, ops: &[StatOp]) {
    let mut saved = (
        p.saved_dexterity,
        p.saved_charisma,
        p.saved_strength,
        p.saved_endurance,
    );
    apply_ops_common(
        &mut p.hp_internal,
        &mut p.periodic_damage,
        &mut p.dexterity_current,
        &mut p.charisma_current,
        &mut p.strength_current,
        &mut p.endurance_current,
        &mut p.available_xp,
        &mut p.temp_effect_timer,
        &mut p.temp_effect_id,
        &mut saved,
        &mut p.dead,
        ops,
    );
    p.saved_dexterity = saved.0;
    p.saved_charisma = saved.1;
    p.saved_strength = saved.2;
    p.saved_endurance = saved.3;
    apply_skill_ops(
        &mut p.healing,
        &mut p.building,
        &mut p.smithing,
        &mut p.trade,
        &mut p.identification,
        ops,
    );
}

/// Apply potion-use operations to a companion [`Entity`].
///
/// `Entity+0x03` bit `0x80` is decoded into the local dead flag, mutated by
/// [`StatOp::Revive`], then cleared again; the other flag bits are preserved.
pub fn apply_entity_ops(e: &mut Entity, ops: &[StatOp]) {
    let mut saved = (
        e.stats.saved_dexterity,
        e.stats.saved_charisma,
        e.stats.saved_strength,
        e.stats.saved_endurance,
    );
    let mut dead = e.flags_b & 0x80 != 0;
    apply_ops_common(
        &mut e.stats.hp_internal,
        &mut e.stats.periodic_damage,
        &mut e.stats.dexterity_current,
        &mut e.stats.charisma_current,
        &mut e.stats.strength_current,
        &mut e.stats.endurance_current,
        &mut e.stats.available_xp,
        &mut e.stats.temp_effect_timer,
        &mut e.stats.temp_effect_id,
        &mut saved,
        &mut dead,
        ops,
    );
    e.stats.saved_dexterity = saved.0;
    e.stats.saved_charisma = saved.1;
    e.stats.saved_strength = saved.2;
    e.stats.saved_endurance = saved.3;
    apply_skill_ops(
        &mut e.stats.healing,
        &mut e.stats.building,
        &mut e.stats.smithing,
        &mut e.stats.trade,
        &mut e.stats.identification,
        ops,
    );
    if dead {
        e.flags_b &= !0x80;
    }
}

/// Decrement one temporary-effect timer and restore the saved stats when it
/// reaches zero. Returns whether the effect expired this frame.
fn restore_temp(
    timer: &mut u16,
    effect: &mut u16,
    dex: &mut u16,
    cha: &mut u16,
    str_: &mut u16,
    end: &mut u16,
    saved: (u16, u16, u16, u16),
) -> bool {
    if *timer == 0 {
        return false;
    }
    *timer = (*timer).saturating_sub(1);
    if *timer == 0 {
        *dex = saved.0;
        *cha = saved.1;
        *str_ = saved.2;
        *end = saved.3;
        *effect = 0;
        return true;
    }
    false
}

/// Advance a temporary potion effect by one logic frame.
pub fn tick_player_temp(p: &mut PlayerCharacter) -> bool {
    restore_temp(
        &mut p.temp_effect_timer,
        &mut p.temp_effect_id,
        &mut p.dexterity_current,
        &mut p.charisma_current,
        &mut p.strength_current,
        &mut p.endurance_current,
        (
            p.saved_dexterity,
            p.saved_charisma,
            p.saved_strength,
            p.saved_endurance,
        ),
    )
}

/// Advance a companion's temporary potion effect by one logic frame.
pub fn tick_entity_temp(e: &mut Entity) -> bool {
    restore_temp(
        &mut e.stats.temp_effect_timer,
        &mut e.stats.temp_effect_id,
        &mut e.stats.dexterity_current,
        &mut e.stats.charisma_current,
        &mut e.stats.strength_current,
        &mut e.stats.endurance_current,
        (
            e.stats.saved_dexterity,
            e.stats.saved_charisma,
            e.stats.saved_strength,
            e.stats.saved_endurance,
        ),
    )
}

/// Try to merge `incoming` into an existing class-12 stack. On success the
/// incoming instance is marked `class = 0xFF` and the caller must not insert it.
pub fn try_stack_ammo(res: &mut Res, slots: &mut SlotsMut, incoming: u16) -> bool {
    if incoming == 0 {
        return false;
    }
    let Some(key) = res.item_stack_key(incoming) else {
        return false;
    };
    if key.0 != 12 {
        return false;
    }
    let Some(qty) = res.item_quantity(incoming) else {
        return false;
    };
    if qty == 0 {
        return false;
    }
    let mut candidates = Vec::new();
    if *slots.special != 0 && *slots.special != incoming {
        candidates.push(*slots.special);
    }
    candidates.extend(
        slots
            .inventory
            .iter()
            .copied()
            .filter(|&id| id != 0 && id != incoming),
    );
    for target in candidates {
        if res.item_stack_key(target) == Some(key) {
            res.add_item_quantity(target, qty);
            res.set_item_class(incoming, 0xFF);
            return true;
        }
    }
    false
}

/// True when the instance is broken/consumed: `class == 0xFF`, or a durable
/// class (`0..=5`) whose current durability is `<= 0`.
pub fn is_broken(res: &Res, id: u16) -> bool {
    let Some(inst) = res.item_instance(id) else {
        return true;
    };
    inst.class == 0xFF
        || (inst.class <= 5 && inst.max_durability > 0.0 && inst.current_durability <= 0.0)
}

/// Remove every broken item from all four containers.
pub fn sweep_broken(res: &Res, slots: &mut SlotsMut) -> Vec<u16> {
    let mut removed = Vec::new();
    let check = |id: u16| -> bool { is_broken(res, id) };
    for i in 0..MAIN_SLOTS {
        let id = slots.equipment[i];
        if id != 0 && check(id) {
            slots.equipment[i] = 0;
            removed.push(id);
        }
    }
    for i in 0..ADDITIONAL_SLOTS {
        let id = slots.additional[i];
        if id != 0 && check(id) {
            slots.additional[i] = 0;
            removed.push(id);
        }
    }
    for i in 0..INVENTORY_SLOTS {
        let id = slots.inventory[i];
        if id != 0 && check(id) {
            slots.inventory[i] = 0;
            removed.push(id);
        }
    }
    let id = *slots.special;
    if id != 0 && check(id) {
        *slots.special = 0;
        removed.push(id);
    }
    removed
}

#[cfg(test)]
mod tests;
