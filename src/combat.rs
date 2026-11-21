//! Original melee combat rules recovered from `konung.res`.
//!
//! Verified entry points:
//! - `FUN_0041b060` hit chance (`rand()%101` vs a reaction roll).
//! - `FUN_0041b42c` reaction roll (human melee/archery, monster dexterity).
//! - `FUN_0041aea8` attack damage (weapon modifier/rating + STR*HP term).
//! - `FUN_0041c3cc` facing dodge, armor/endurance mitigation, poison apply and
//!   equipment wear.
//! - `FUN_0041ab7c` armor total (base armor + equipment slots 3..=5).
//!
//! Numeric constants are the original DGROUP doubles at `0x450000`; the facing
//! dodge table is the 8x8 byte table at `0x450DB4`.

use crate::character::PlayerCharacter;
use crate::item::ItemDb;
use crate::population::Entity;

pub mod loot;
pub mod los;
pub mod projectile;

/// Original LCG (`FUN_0043ea46`): `seed = seed*0x41c64e6d + 0x3039`,
/// result `(seed >> 16) & 0x7FFF`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OriginalRng {
    state: u32,
}

impl OriginalRng {
    /// Seed the generator; `0` is replaced by `1` (the all-zero state of an LCG
    /// would never advance).
    pub fn new(seed: u32) -> OriginalRng {
        OriginalRng {
            state: if seed == 0 { 1 } else { seed },
        }
    }

    /// Advance one `0x41c64e6d`/`0x3039` step and return the high 15 bits.
    pub fn next_u32(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(0x41c6_4e6d).wrapping_add(0x3039);
        (self.state >> 16) & 0x7FFF
    }

    /// `rand() % 101` as used by every combat roll.
    pub fn roll(&mut self) -> u32 {
        self.next_u32() % 101
    }
}

impl Default for OriginalRng {
    /// A generator seeded with `1`, matching the original's fixed start state.
    fn default() -> Self {
        OriginalRng::new(1)
    }
}

/// `0x450DB4`: `[defender_facing * 8 + attacker_facing] != 0` enables the
/// dexterity dodge roll.
pub const FACING_DODGE: [[u8; 8]; 8] = [
    [0, 0, 0, 1, 1, 1, 0, 0],
    [0, 0, 0, 0, 0, 1, 1, 1],
    [0, 0, 0, 0, 0, 0, 0, 1],
    [0, 1, 0, 1, 0, 0, 0, 0],
    [0, 0, 0, 1, 0, 1, 0, 1],
    [0, 1, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 1, 0, 1, 0, 1],
    [0, 0, 0, 1, 0, 0, 0, 0],
];

/// `0x450178` - dodge threshold scale (`dex * 0.5`).
pub const DODGE_DEX_SCALE: f64 = 0.5;
/// `0x450180` - incoming damage weight before armor.
pub const MITIGATE_ATTACK_SCALE: f64 = 0.7;
/// `0x450188` - unarmored incoming damage weight.
pub const MITIGATE_PIERCE_SCALE: f64 = 0.3;
/// `0x450190` - final endurance divisor multiplier.
pub const MITIGATE_ENDURANCE_SCALE: f64 = 16.0;
/// `0x450198` - armor durability wear scale.
pub const ARMOR_WEAR_SCALE: f64 = 0.001;
/// `0x4501a0` - weapon durability wear scale.
pub const WEAPON_WEAR_SCALE: f64 = 0.0001;

/// Human unarmed damage: `round(STR * 0.02 * HP * 0.0625)` = `STR*HP/800`.
pub const HUMAN_UNARMED_STR_HP: f64 = 0.02 * 0.0625;
/// Human armed bonus: `round(STR * 0.04 * HP * 0.0625)` = `STR*HP/400`
/// (weapon value is added on top).
pub const HUMAN_ARMED_STR_HP: f64 = 0.04 * 0.0625;
/// Monster damage: `round(STR * 0.04 * HP * 0.0625)` = `STR*HP/400`.
pub const MONSTER_STR_HP: f64 = 0.04 * 0.0625;
/// `0x4500e4` / `0x45010c` / `0x450114` and the reaction constants.
pub const REACTION_SKILL_SCALE: f64 = 0.00015;
/// `0x4500e4` / `0x45010c` / `0x450114` reaction group - human reaction base
/// (`1.5`) before the mastery/skill term.
pub const REACTION_BASE: f64 = 1.5;
/// Reaction group - archery reaction scale (`0.7`), used by the bow branch.
pub const ARCHERY_REACTION_SCALE: f64 = 0.7;
/// Reaction group - monster reaction base (`50`).
pub const MONSTER_REACTION_BASE: f64 = 50.0;
/// Monster reaction dexterity weight (`0.25` per point of base dexterity).
pub const MONSTER_REACTION_DEX: f64 = 0.25;

/// x87 `FISTP` round-to-nearest-even helper (`FUN_0043EAAE`); non-finite
/// inputs collapse to `0` instead of trapping.
fn round_even(v: f64) -> i64 {
    if !v.is_finite() {
        return 0;
    }
    v.round_ties_even() as i64
}

/// Runtime target handle used by the melee orchestration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CombatTarget {
    /// Party slot 0 (`PlayableApp.player`).
    Player,
    /// Recruited companion index in `PlayableApp.party` (slot `index + 1`).
    Party(usize),
    /// World actor source `Entity` id.
    World(usize),
    /// Prop/object table index (`Entity+0x06` negative target encoding).
    Prop(usize),
}

/// The combat-relevant view of an Entity / PlayerCharacter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CombatStats {
    /// `Entity+0x04` archetype byte; bit `0x40` selects the object/monster family.
    pub archetype: u8,
    /// `Entity+0x12` active weapon/equipment slot.
    pub weapon_slot: u8,
    /// `Entity+0x5C` internal HP (1600 at new game; displayed as `(hp+15)/16`).
    pub hp: u16,
    /// `Entity+0x54` dexterity base.
    pub dexterity: u16,
    /// `Entity+0x5E` strength base.
    pub strength: u16,
    /// `Entity+0x62` endurance base; divides the mitigation term.
    pub endurance: u16,
    /// `Entity+0x4E` innate armor, before the slots 3..=5 item ratings.
    pub base_armor: u16,
    /// `Entity+0x40` melee mastery skill.
    pub melee_mastery: u16,
    /// `Entity+0x44` archery mastery skill.
    pub archery_mastery: u16,
    /// `Entity+0x50` periodic damage (poison), also the stinger loot value.
    pub periodic_damage: u16,
    /// `Entity+0xC4` armor set bonus (`FUN_0041C69C`; the app recomputes it).
    pub armor_bonus: i16,
    /// `Entity+0xC6` damage set bonus (`FUN_0041C69C`).
    pub damage_bonus: i16,
    /// `Entity+0xC8` attack/reaction set bonus (`FUN_0041C69C`).
    pub attack_bonus: i16,
    /// `Entity+0xD1` add-vs-replace flags (bit 0 armor, bit 1 damage, bit 2 attack).
    pub bonus_flags: u8,
    /// `Entity+0xCC` ammo (`+0xCC` is `special_equipment`): the arrow instance
    /// whose modifier/subtype/poison `FUN_0041BF44` copies into a projectile.
    pub ammo: u16,
    /// `Entity+0xCE` temporary-effect timer in logic frames.
    pub temp_effect_timer: u16,
    /// `Entity+0xCA` temporary-effect id (`+0xCA` is added to `0x41B42C`).
    pub temp_effect_id: u16,
    /// `Entity+0x66` six equipped item instance ids (`0` = empty slot).
    pub equipment: [u16; 6],
}

impl CombatStats {
    /// Build the stats view from the runtime protagonist.
    pub fn from_player(p: &PlayerCharacter) -> CombatStats {
        CombatStats {
            archetype: p.entity_class(),
            weapon_slot: p.raw_0x12,
            hp: p.hp_internal,
            // The original combat formulas read the BASE fields
            // (`Entity+0x54/+0x5E/+0x62`), not the derived current fields.
            dexterity: p.dexterity_base,
            strength: p.strength_base,
            endurance: p.endurance_base,
            base_armor: p.base_armor,
            melee_mastery: p.melee_mastery,
            archery_mastery: p.archery_mastery,
            periodic_damage: p.periodic_damage,
            armor_bonus: p.armor_bonus,
            damage_bonus: p.damage_bonus,
            attack_bonus: p.attack_bonus,
            bonus_flags: p.bonus_flags,
            ammo: p.special_equipment,
            temp_effect_timer: p.temp_effect_timer,
            temp_effect_id: p.temp_effect_id,
            equipment: p.equipment,
        }
    }

    /// Build the stats view from one authored/world `Entity` record.
    pub fn from_entity(e: &Entity) -> CombatStats {
        CombatStats {
            archetype: e.archetype,
            weapon_slot: e.raw_0x12,
            hp: e.stats.hp_internal,
            // `FUN_0041b42c` / `FUN_0041aea8` / `FUN_0041c3cc` read the base
            // fields (`Entity+0x54/+0x5E/+0x62`).
            dexterity: e.stats.dexterity_base,
            strength: e.stats.strength_base,
            endurance: e.stats.endurance_base,
            base_armor: e.stats.base_armor,
            melee_mastery: e.stats.melee_mastery,
            archery_mastery: e.stats.archery_mastery,
            periodic_damage: e.stats.periodic_damage,
            armor_bonus: e.stats.armor_bonus,
            damage_bonus: e.stats.damage_bonus,
            attack_bonus: e.stats.attack_bonus,
            bonus_flags: e.stats.bonus_flags,
            ammo: e.special_equipment,
            temp_effect_timer: e.stats.temp_effect_timer,
            temp_effect_id: e.stats.temp_effect_id,
            equipment: e.equipment,
        }
    }

    /// Whether this actor uses the object/monster family (`archetype & 0x40`).
    pub fn is_object_family(&self) -> bool {
        self.archetype & 0x40 != 0
    }

    /// Active melee weapon equipment slot (`0x12` in `{0,2}` and non-zero id).
    pub fn weapon_slot_index(&self) -> Option<usize> {
        let slot = self.weapon_slot as usize;
        if (slot == 0 || slot == 2) && self.equipment.get(slot).copied().unwrap_or(0) != 0 {
            Some(slot)
        } else {
            None
        }
    }
}

/// Pure inputs of `FUN_0041b42c` (`reaction`/hit value).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReactionInput {
    /// `Entity+0x04 & 0x40` object/monster family.
    pub object_family: bool,
    /// Derived dexterity (`Entity+0x54`).
    pub dexterity: u16,
    /// `Entity+0x12 == 1` (bow active).
    pub bow: bool,
    /// `Entity+0x40` melee or `+0x44` archery mastery.
    pub skill: u16,
    /// `param_2`: `0` for the melee hit test, `1` for the stat-9 query.
    pub distance: i32,
    /// The active bow prototype's `u16@+0x34` reach.
    pub bow_range: u16,
    /// `Entity+0xCE` temporary-effect timer.
    pub temp_timer: u16,
    /// `Entity+0xCA` temporary-effect id.
    pub temp_id: u16,
    /// `Entity+0xC8` attack set bonus.
    pub attack_bonus: i16,
    /// `Entity+0xD1` add-vs-replace flags (bit 2 = attack).
    pub bonus_flags: u8,
}

/// `FUN_0041b42c`: reaction / hit value 0..100.
///
/// Object family: `round(50 + 0.25*dex)` (`0x45006C` = 0.5, `0x450074` = 100).
/// Humans: `round(100 - (100 - 0.5*dex) / (1.5 + 0.00015*skill))`; with a bow
/// active the numerator is multiplied by `0.7 * (1 + distance/range)`
/// (`0x45007C` = 0.7, `0x450084` = 0.5, `0x45008C` = 100, `0x450094` = 0.00015,
/// `0x45009C` = 1.5, `0x4500A4` = 0.5, `0x4500AC` = 100, `0x4500B4` = 0.00015,
/// `0x4500BC` = 1.5). While a temporary effect runs `+0xCA` is added, then the
/// `+0xC8` set bonus replaces or adds by `+0xD1 & 4`.
pub fn reaction_of(i: &ReactionInput) -> i32 {
    let dex = i.dexterity as f64;
    let mut value = if i.object_family {
        round_even(MONSTER_REACTION_BASE + dex * MONSTER_REACTION_DEX) as i32
    } else {
        let denom = REACTION_BASE + REACTION_SKILL_SCALE * i.skill as f64;
        let dext = 100.0 - DODGE_DEX_SCALE * dex;
        if i.bow {
            let range = i.bow_range.max(1) as f64;
            let factor =
                ARCHERY_REACTION_SCALE * (1.0 + i.distance as f64 / range);
            round_even(100.0 - factor * dext / denom) as i32
        } else {
            round_even(100.0 - dext / denom) as i32
        }
    };
    if i.temp_timer != 0 {
        value += i.temp_id as i32;
    }
    if i.attack_bonus != 0 {
        if i.bonus_flags & 4 == 0 {
            value = i.attack_bonus as i32;
        } else {
            value += i.attack_bonus as i32;
        }
    }
    value
}

/// `FUN_0041b42c`: reaction roll used by the hit test.
///
/// `distance` is `0` for the melee hit test; `bow_range` is the active bow's
/// prototype reach (only read when `Entity+0x12 == 1`).
pub fn reaction(s: &CombatStats, distance: i32, bow_range: u16) -> i32 {
    let bow = s.weapon_slot == 1;
    reaction_of(&ReactionInput {
        object_family: s.is_object_family(),
        dexterity: s.dexterity,
        bow,
        skill: if bow {
            s.archery_mastery
        } else {
            s.melee_mastery
        },
        distance,
        bow_range,
        temp_timer: s.temp_effect_timer,
        temp_id: s.temp_effect_id,
        attack_bonus: s.attack_bonus,
        bonus_flags: s.bonus_flags,
    })
}

/// `FUN_0041b42c` bow reach: the active bow prototype's `u16@+0x34`.
pub fn bow_range(db: &ItemDb, save: &[u8], s: &CombatStats) -> u16 {
    if s.weapon_slot != 1 {
        return 0;
    }
    let Some(id) = s.equipment.get(1).copied() else {
        return 0;
    };
    let Some(inst) = ItemDb::decode_instance(save, id) else {
        return 0;
    };
    db.info(inst.class, inst.subtype)
        .map(|i| i.range)
        .unwrap_or(0)
}

/// `FUN_0041b060`: `reaction < rand()%101` misses.
pub fn roll_hit(attacker: &CombatStats, bow_range: u16, rng: &mut OriginalRng) -> bool {
    let roll = rng.roll() as i32;
    reaction(attacker, 0, bow_range) >= roll
}

/// Resolve the active melee weapon's slot, decoded instance and prototype.
///
/// Returns `None` when the active slot is not a melee slot, the id is `0`, or
/// the save/prototype lookup fails.
fn weapon_info<'a>(
    db: &'a ItemDb,
    save: &[u8],
    s: &CombatStats,
) -> Option<(usize, crate::item::ItemInstance, crate::item::ItemInfo)> {
    let slot = s.weapon_slot_index()?;
    let id = s.equipment[slot];
    let inst = ItemDb::decode_instance(save, id)?;
    let info = db.info(inst.class, inst.subtype)?;
    Some((slot, inst, info))
}

/// `FUN_0041aea8`: raw attack damage and the poison value added on hit.
pub fn attack_damage(
    db: &ItemDb,
    save: &[u8],
    s: &CombatStats,
    target_archetype: u8,
) -> (i32, u16) {
    if s.is_object_family() {
        let dmg = round_even(s.strength as f64 * s.hp as f64 * MONSTER_STR_HP) as i32;
        return (dmg.max(0), s.periodic_damage);
    }
    let unarmed = round_even(s.strength as f64 * s.hp as f64 * HUMAN_UNARMED_STR_HP) as i32;
    let (mut value, poison) = if let Some((_, inst, info)) = weapon_info(db, save, s) {
        let base = db
            .modifier_damage(inst.modifier_index, target_archetype)
            .unwrap_or(info.rating) as i32;
        let armed_bonus = round_even(s.strength as f64 * s.hp as f64 * HUMAN_ARMED_STR_HP) as i32;
        (base + armed_bonus, inst.poison)
    } else {
        (unarmed, 0)
    };
    // `FUN_0041aea8`: the `+0xC6` damage set bonus (humans only).
    if s.damage_bonus != 0 {
        if s.bonus_flags & 2 == 0 {
            value = s.damage_bonus as i32;
        } else {
            value += s.damage_bonus as i32;
        }
    }
    (value, poison)
}

/// `FUN_0041ab7c`: base armor plus the rating of equipment slots 3..=5, then
/// the `+0xC4` set-bonus term (`+0xD1 & 1` clear replaces, otherwise adds).
pub fn armor_total(db: &ItemDb, save: &[u8], s: &CombatStats) -> i32 {
    let mut armor = s.base_armor as i32;
    for slot in 3..6 {
        let id = s.equipment[slot];
        if id == 0 {
            continue;
        }
        if let Some(inst) = ItemDb::decode_instance(save, id) {
            if let Some(info) = db.info(inst.class, inst.subtype) {
                armor += info.rating as i32;
            }
        }
    }
    if s.armor_bonus != 0 {
        if s.bonus_flags & 1 == 0 {
            armor = s.armor_bonus as i32;
        } else {
            armor += s.armor_bonus as i32;
        }
    }
    armor
}

/// `FUN_0041c3cc` mitigation:
/// `round(16 * (1 + (damage*0.3 + max(0, damage*0.7 - armor)) / endurance))`.
pub fn mitigate_damage(s: &CombatStats, incoming: i32, armor: i32) -> u16 {
    let incoming = incoming.max(0) as f64;
    let temp = (incoming * MITIGATE_ATTACK_SCALE - armor as f64).max(0.0);
    let endurance = (s.endurance.max(1)) as f64;
    let value =
        MITIGATE_ENDURANCE_SCALE * (1.0 + (incoming * MITIGATE_PIERCE_SCALE + temp) / endurance);
    round_even(value).clamp(0, u16::MAX as i64) as u16
}

/// Snapshot of the projectile payload copied by `FUN_0041BF44`.
///
/// `FUN_0041BF44` stores the **bow** class rating at `+0x16` but the **ammo**
/// instance fields at `+0x18` (poison), `+0x1E` (modifier index) and `+0x1F`
/// (subtype); `FUN_0041AC40` uses them to pick the damage base.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BowShot {
    /// Prototype combat value of the **bow** (`u16@0x459B7C + subtype*0x40`).
    pub rating: u16,
    /// Ammo instance `+0x02` weapon-modifier index (`-1` = none).
    pub modifier: i8,
    /// Ammo instance `+0x03` subtype; selects the `0x459B7C` rating row.
    pub subtype: u8,
    /// Ammo instance `+0x0C` poison added to the target on a hit.
    pub poison: u16,
}

/// `FUN_0041bf44`: bow rating + the ammo's modifier/subtype/poison.
pub fn bow_shot(db: &ItemDb, save: &[u8], s: &CombatStats) -> Option<BowShot> {
    if s.weapon_slot != 1 {
        return None;
    }
    let id = *s.equipment.get(1)?;
    if id == 0 || s.ammo == 0 {
        return None;
    }
    let bow_inst = ItemDb::decode_instance(save, id)?;
    let bow_info = db.info(bow_inst.class, bow_inst.subtype)?;
    if bow_info.class != 1 || bow_info.range == 0 {
        return None;
    }
    let ammo = ItemDb::decode_instance(save, s.ammo)?;
    Some(BowShot {
        rating: bow_info.rating,
        modifier: ammo.modifier_index,
        subtype: ammo.subtype,
        poison: ammo.poison,
    })
}

/// `FUN_0041AC40`: projectile damage = `(modifier or ammo rating) * bow rating`
/// plus the shooter's `+0xC6` set bonus.
pub fn ranged_damage(
    db: &ItemDb,
    bow: &BowShot,
    target_archetype: u8,
    owner: &CombatStats,
) -> (i32, u16) {
    // The original treats a modifier below `1` as "none" for the projectile
    // (unlike the melee path, which accepts `0`).
    let base = if bow.modifier >= 1 {
        db.modifier_damage(bow.modifier, target_archetype)
            .unwrap_or_else(|| db.rating_for_subtype(bow.subtype))
    } else {
        db.rating_for_subtype(bow.subtype)
    } as i64;
    let mut raw = base * bow.rating as i64;
    if owner.damage_bonus != 0 {
        if owner.bonus_flags & 2 == 0 {
            raw = owner.damage_bonus as i64;
        } else {
            raw += owner.damage_bonus as i64;
        }
    }
    (raw.clamp(0, i32::MAX as i64) as i32, bow.poison)
}

/// `FUN_0041B658`: kill power used for ranged money/archery rewards and the
/// corpse-loot `bonus` argument.
pub fn combat_power(db: &ItemDb, save: &[u8], s: &CombatStats) -> i32 {
    let armor = armor_total(db, save, s) as f64;
    let mut power =
        armor * 0.1 + s.dexterity as f64 * 0.2 + s.strength as f64 * 0.3 + s.endurance as f64 * 0.3;
    if !s.is_object_family() {
        power += s.melee_mastery as f64 * 0.002 + s.archery_mastery as f64 * 0.003;
        if let Some(slot) = s.weapon_slot_index() {
            let id = s.equipment[slot];
            let rating = ItemDb::decode_instance(save, id)
                .and_then(|i| db.info(i.class, i.subtype))
                .map(|i| i.rating)
                .unwrap_or(0);
            power = rating as f64 * 0.2 + power;
        }
    }
    round_even(power).max(0) as i32
}

/// Projectile damage application (`FUN_0041C1E8`): dexterity dodge then the
/// same armor/endurance mitigation as melee (no facing table, no weapon wear).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectileRoll {
    /// `false` only when the dexterity dodge succeeded.
    pub hit: bool,
    /// The `rand()%101 <= dex*0.5` dodge roll succeeded.
    pub dodged: bool,
    /// Final damage after armor/endurance mitigation, applied to HP.
    pub damage: u16,
    /// Raw `FUN_0041AC40` damage before mitigation (also the armor-wear input).
    pub incoming: u16,
    /// `damage >= defender.hp`, i.e. the shot kills.
    pub lethal: bool,
    /// Poison carried by the arrow, applied on a hit.
    pub poison: u16,
}

/// Resolve one arrow hit against a defender: dexterity dodge, then the melee
/// armor/endurance mitigation (no facing table, no weapon wear).
pub fn resolve_projectile(
    db: &ItemDb,
    save: &[u8],
    bow: &BowShot,
    owner: &CombatStats,
    defender: &CombatStats,
    rng: &mut OriginalRng,
) -> ProjectileRoll {
    if (rng.roll() as i64) <= round_even(defender.dexterity as f64 * DODGE_DEX_SCALE) {
        return ProjectileRoll {
            hit: false,
            dodged: true,
            damage: 0,
            incoming: 0,
            lethal: false,
            poison: 0,
        };
    }
    let (raw, poison) = ranged_damage(db, bow, defender.archetype, owner);
    let armor = armor_total(db, save, defender);
    let damage = mitigate_damage(defender, raw, armor);
    ProjectileRoll {
        hit: true,
        dodged: false,
        damage,
        incoming: raw.clamp(0, u16::MAX as i32) as u16,
        lethal: damage >= defender.hp,
        poison,
    }
}

/// Complete one melee attack (`FUN_0041b060` -> `FUN_0041aea8` ->
/// `FUN_0041c3cc`) without applying HP or durability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MeleeRoll {
    /// `false` when the reaction hit test or the facing dodge failed.
    pub hit: bool,
    /// The facing-table + dexterity dodge branch consumed the blow.
    pub dodged: bool,
    /// Final damage after armor/endurance mitigation.
    pub damage: u16,
    /// Raw `FUN_0041aea8` value (before armor/endurance mitigation).
    pub incoming: u16,
    /// `damage >= defender.hp`, i.e. the blow kills.
    pub lethal: bool,
    /// Poison carried by the weapon instance (`+0x0C`); `0` when unarmed.
    pub poison: u16,
}

/// Run the full melee pipeline (`FUN_0041b060` hit test, `FUN_0041aea8`
/// damage, facing table + dexterity dodge, `FUN_0041c3cc` mitigation).
pub fn resolve_melee(
    db: &ItemDb,
    save: &[u8],
    attacker: &CombatStats,
    attacker_facing: u8,
    defender: &CombatStats,
    defender_facing: u8,
    rng: &mut OriginalRng,
) -> MeleeRoll {
    let range = bow_range(db, save, attacker);
    if !roll_hit(attacker, range, rng) {
        return MeleeRoll {
            hit: false,
            dodged: false,
            damage: 0,
            incoming: 0,
            lethal: false,
            poison: 0,
        };
    }
    let (raw, poison) = attack_damage(db, save, attacker, defender.archetype);
    if FACING_DODGE[(defender_facing & 7) as usize][(attacker_facing & 7) as usize] != 0
        && (rng.roll() as i64) <= round_even(defender.dexterity as f64 * DODGE_DEX_SCALE)
    {
        return MeleeRoll {
            hit: false,
            dodged: true,
            damage: 0,
            incoming: 0,
            lethal: false,
            poison: 0,
        };
    }
    let armor = armor_total(db, save, defender);
    let damage = mitigate_damage(defender, raw, armor);
    MeleeRoll {
        hit: true,
        dodged: false,
        damage,
        incoming: raw.clamp(0, u16::MAX as i32) as u16,
        lethal: damage >= defender.hp,
        poison,
    }
}

/// Armor durability loss for one landed hit: `min(incoming/armor, 1) *
/// item_rating * 0.001` on slots 3..=5.
pub fn armor_wear(incoming: i32, armor: i32, rating: u16) -> f32 {
    let ratio = if armor <= 0 {
        1.0
    } else {
        (incoming as f64 / armor as f64).min(1.0)
    };
    (ratio * rating as f64 * ARMOR_WEAR_SCALE) as f32
}

/// Weapon durability loss for one landed hit: `incoming * 0.0001`.
pub fn weapon_wear(incoming: i32) -> f32 {
    (incoming.max(0) as f64 * WEAPON_WEAR_SCALE) as f32
}

#[cfg(test)]
mod tests;
