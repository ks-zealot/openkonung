//! `player` subsystem of [`crate::character`] (split from `character.rs`).

use super::*;

/// The one authoritative runtime player: hero identity + class + persistent
/// character-creation values. Field offsets correspond to the original `Entity`
/// fields (`+04`, `+40..+4C`, `+54/+56`, `+58/+5A`, `+5E/+60`, `+62/+64`) but
/// this is not a full 0x100 `Entity` record — the runtime player is not an
/// `Entity` in the remake.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerCharacter {
    pub hero: HeroTemplate,
    pub class: CharacterClass,
    pub level: u8,
    pub money: i32,
    pub available_xp: u32,
    pub invested_xp: u32,

    pub melee_mastery: u16,
    pub trade: u16,
    pub archery_mastery: u16,
    pub building: u16,
    pub healing: u16,
    pub identification: u16,
    pub smithing: u16,

    pub dexterity_base: u16,
    pub dexterity_current: u16,
    pub charisma_base: u16,
    pub charisma_current: u16,
    pub strength_base: u16,
    pub strength_current: u16,
    pub endurance_base: u16,
    pub endurance_current: u16,

    /// `Entity+0x66` — 6 equipped item instance ids (kept separate from the
    /// inventory). Attached from the starting `Entity[0]` at game start.
    pub equipment: [u16; 6],
    /// `Entity+0x72` — 32 inventory item instance ids.
    pub inventory: [u16; 32],
    /// `Entity+0xB2` — 5 additional equipment item instance ids.
    pub additional_equipment: [u16; 5],
    /// `Entity+0xCC` — special equipment item instance id.
    pub special_equipment: u16,
    /// `Entity+0x5C` — HP (internal representation; 1600 at new game).
    pub hp_internal: u16,
    /// `Entity+0x50` — periodic damage (rendered as «Отравление»/poisoning).
    pub periodic_damage: u16,
    /// `Entity+0x4E` — base armor (equipped-item contributions are added on top).
    pub base_armor: u16,
    /// `Entity+0x12` — character-creation field used by the character screen's
    /// overlapping equipment-slot branch.
    pub raw_0x12: u8,
    /// `Entity+0x03` — status flags; bit `0x02` selects the AUTO button state
    /// (`INTR[170]` when set, else `INTR[169]`).
    pub flags_b: u8,
    /// `Entity+0x04` bit `0x80` — dead/incapacitated. Forces the dead portrait
    /// variant (`FUN_0042C730`, set by the defeat path `FUN_0041D048`).
    pub dead: bool,
}


impl PlayerCharacter {
    /// Copy a [`CharacterBuild`] once into the runtime player. Base == current
    /// for every primary stat, and the class preset is **not** re-applied.
    pub fn from_build(hero: HeroTemplate, build: CharacterBuild) -> PlayerCharacter {
        PlayerCharacter {
            hero,
            class: build.class,
            level: 1,
            money: 0,
            available_xp: build.available_xp,
            invested_xp: 0,
            melee_mastery: build.melee_mastery,
            trade: build.trade,
            archery_mastery: build.archery_mastery,
            building: build.building,
            healing: build.healing,
            identification: build.identification,
            smithing: build.smithing,
            dexterity_base: build.dexterity,
            dexterity_current: build.dexterity,
            charisma_base: build.charisma,
            charisma_current: build.charisma,
            strength_base: build.strength,
            strength_current: build.strength,
            endurance_base: build.endurance,
            endurance_current: build.endurance,
            equipment: [0; 6],
            inventory: [0; 32],
            additional_equipment: [0; 5],
            special_equipment: 0,
            hp_internal: 0,
            periodic_damage: 0,
            base_armor: 0,
            raw_0x12: 0,
            flags_b: 0,
            dead: false,
        }
    }


    /// Copy the authored starting equipment/inventory from the protagonist
    /// `Entity[0]` record. Called once at game start; never re-derived on redraw.
    pub fn attach_starting_items(&mut self, entity: &crate::population::Entity) {
        self.equipment = entity.equipment;
        self.inventory = entity.inventory;
        self.additional_equipment = entity.additional_equipment;
        self.special_equipment = entity.special_equipment;
        // `+0x5C` health and `+0x12` are restored from the loaded Game.N entity
        // by the original new-game path (the equipment/inventory are not).
        self.hp_internal = entity.stats.hp_internal;
        self.periodic_damage = entity.stats.periodic_damage;
        self.base_armor = entity.stats.base_armor;
        self.raw_0x12 = entity.raw_0x12;
        // Money (`+0x34`) is likewise taken from the loaded Game.N entity (the
        // field-restore does not overwrite it).
        self.money = entity.stats.money;
        self.flags_b = entity.flags_b;
    }


    /// AUTO combat button state: `Entity+0x03 & 0x02`.
    pub fn auto_enabled(&self) -> bool {
        self.flags_b & 0x02 != 0
    }


    /// `Entity+0xD6` portrait group (the hero's visual index).
    pub fn portrait_group(&self) -> u8 {
        self.hero.visual_index
    }


    /// `Entity+0x04` status/appearance byte used by the portrait selector:
    /// low 3 bits = appearance, bit `0x80` = dead/special (`FUN_0042C730`).
    pub fn portrait_status(&self) -> u8 {
        self.entity_class() | if self.dead { 0x80 } else { 0 }
    }


    /// Iterate the 32 inventory slots exactly as authored (including empty `0`s).
    pub fn inventory(&self) -> &[u16; 32] {
        &self.inventory
    }


    /// `Entity+0x04` renderer class value.
    pub fn entity_class(&self) -> u8 {
        self.class.entity_class()
    }


    /// Resource palette id from the hero identity.
    pub fn palette_id(&self) -> i32 {
        self.hero.palette_id()
    }


    /// Human-readable player state block (diagnostic only, task §9).
    pub fn debug_block(&self) -> String {
        format!(
            "HERO {}\nCLASS {} (entity +04 = {})\n\nlevel={}\npoints={}\n\nSTR base={} current={}\nDEX base={} current={}\nCHA base={} current={}\nEND base={} current={}\n\nmax_load={:.1} max_party={}\n\nmelee={}\narchery={}\ntrade={}\nbuilding={}\nhealing={}\nidentify={}\nsmithing={}\n",
            self.hero.name,
            self.class.name(),
            self.entity_class(),
            self.level,
            self.available_xp,
            self.strength_base,
            self.strength_current,
            self.dexterity_base,
            self.dexterity_current,
            self.charisma_base,
            self.charisma_current,
            self.endurance_base,
            self.endurance_current,
            max_load(self.endurance_base),
            max_party(self.class, self.charisma_base),
            self.melee_mastery,
            self.archery_mastery,
            self.trade,
            self.building,
            self.healing,
            self.identification,
            self.smithing,
        )
    }

}
