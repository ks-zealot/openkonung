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
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
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
/// `0x4500e4` - human reaction base value before the mastery/skill term.
pub const REACTION_BASE: f64 = 1.5;
/// `0x45010c` - archery reaction skill scale (`0.7`).
pub const ARCHERY_REACTION_SCALE: f64 = 0.7;
/// `0x450114` - monster reaction base (`50`).
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
    /// `Entity+0x5C` internal HP (1600 at new game; displayed as `hp/16`+15).
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

/// `FUN_0041b42c`: reaction roll used by the hit test.
pub fn reaction(s: &CombatStats) -> i32 {
    if s.is_object_family() {
        return round_even(MONSTER_REACTION_BASE + s.dexterity as f64 * MONSTER_REACTION_DEX)
            as i32;
    }
    let dex_term = (100.0 - s.dexterity as f64 * 0.5).max(1.0);
    let value = if s.weapon_slot == 1 {
        let denom = ARCHERY_REACTION_SCALE * dex_term;
        (REACTION_BASE + s.archery_mastery as f64 * REACTION_SKILL_SCALE) / denom.max(1e-9)
    } else {
        (REACTION_BASE + s.melee_mastery as f64 * REACTION_SKILL_SCALE) / dex_term.max(1e-9)
    };
    round_even(100.0 - value) as i32
}

/// `FUN_0041b060`: `reaction < rand()%101` misses.
pub fn roll_hit(attacker: &CombatStats, rng: &mut OriginalRng) -> bool {
    let roll = rng.roll() as i32;
    reaction(attacker) >= roll
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
    if let Some((_, inst, info)) = weapon_info(db, save, s) {
        let base = db
            .modifier_damage(inst.modifier_index, target_archetype)
            .unwrap_or(info.rating) as i32;
        let armed_bonus = round_even(s.strength as f64 * s.hp as f64 * HUMAN_ARMED_STR_HP) as i32;
        (base + armed_bonus, inst.poison)
    } else {
        (unarmed, 0)
    }
}

/// `FUN_0041ab7c`: base armor plus the rating of equipment slots 3..=5.
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

/// Snapshot of the bow fields copied into a projectile by `FUN_0041BF44`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BowShot {
    /// Prototype combat value `u16@0x459B7C + subtype*0x40` (`ItemInfo::rating`).
    pub rating: u16,
    /// Item instance `+0x02` weapon-modifier index into `0x462798`
    /// (`-1` = no modifier).
    pub modifier: i8,
    /// Item instance `+0x03` subtype; selects the `0x459B84` probe range.
    pub subtype: u8,
    /// Item instance `+0x0C` poison added to the target on a hit.
    pub poison: u16,
}

/// `FUN_0041bf44`: bow rating/modifier/subtype/poison from `Equipment[1]`.
pub fn bow_shot(db: &ItemDb, save: &[u8], s: &CombatStats) -> Option<BowShot> {
    if s.weapon_slot != 1 {
        return None;
    }
    let id = *s.equipment.get(1)?;
    if id == 0 {
        return None;
    }
    let inst = ItemDb::decode_instance(save, id)?;
    let info = db.info(inst.class, inst.subtype)?;
    if info.class != 1 || info.range == 0 {
        return None;
    }
    Some(BowShot {
        rating: info.rating,
        modifier: inst.modifier_index,
        subtype: inst.subtype,
        poison: inst.poison,
    })
}

/// `FUN_0041AC40`: projectile damage = `(modifier or bow rating) * bow rating`.
pub fn ranged_damage(db: &ItemDb, bow: &BowShot, target_archetype: u8) -> (i32, u16) {
    let base = db
        .modifier_damage(bow.modifier, target_archetype)
        .unwrap_or_else(|| db.rating_for_subtype(bow.subtype)) as i64;
    let raw = base * bow.rating as i64;
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
    let (raw, poison) = ranged_damage(db, bow, defender.archetype);
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
    if !roll_hit(attacker, rng) {
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
