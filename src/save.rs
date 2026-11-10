//! Native, versioned save-game payload for the wasm-only build.
//!
//! The original engine writes a flat `0xB07A4` snapshot to `KONUNG.SA0..9`
//! (`FUN_00424B3C`/`FUN_004246C0`). The browser build has no filesystem, so the
//! remake persists the *same state classes* in a versioned `bincode` payload
//! wrapped in a small header and compressed with deflate. The shell stores the
//! resulting bytes in `localStorage` (one compressed payload per slot).
//!
//! The quest-progress contract mirrors the original B7 block at save offset
//! `0x18`: all 300 four-byte slots are preserved, including the `+0x02`
//! journal-line field, not just the `0x80` active bit.

use std::io::{Read, Write};

use serde::{Deserialize, Serialize};

use crate::actors::PopulationBuild;
use crate::character::{CharacterClass, PlayerCharacter};
use crate::combat::projectile::Projectile;
use crate::combat::OriginalRng;
use crate::hero;
use crate::population::{Entity, Group, Settlement, SettlementTrade};
use crate::world_objects::{ActiveWorldObject, WorldObjectSource};

/// Opaque payload header (`OPK` + format letter).
pub const MAGIC: [u8; 4] = *b"OPKS";
/// Current payload version. Increment on any layout change.
pub const VERSION: u16 = 5;
/// Legacy payload version (NPC `Entity+0xD2` values defaulted on load).
pub const VERSION_V4: u16 = 4;
/// Legacy payload version (apprentice training timers defaulted on load).
pub const VERSION_V3: u16 = 3;
/// Legacy payload version (growth group extras defaulted on load).
pub const VERSION_V2: u16 = 2;
/// Legacy payload version (quest extras defaulted on load).
pub const VERSION_V1: u16 = 1;
/// Item-instance block copied from the loaded `Game.N` (8192 x 16 bytes).
pub const ITEM_TABLE_LEN: usize = 0x20000;
/// `B7` quest-flag slot count.
pub const FLAG_SLOTS: usize = 300;
/// Discovered world-map location flags (`0x81CF80`).
pub const DISCOVERED_LEN: usize = 56;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SaveMeta {
    /// `Game.N` suffix used by this save (0/1/2).
    pub profile: u32,
    /// Hero identity index (0 Dragomir, 1 Mikhail, 2 Wolf).
    pub hero_index: u32,
    /// Current location id.
    pub current_map: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlayerSave {
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
    pub equipment: [u16; 6],
    pub inventory: [u16; 32],
    pub additional_equipment: [u16; 5],
    pub special_equipment: u16,
    pub hp_internal: u16,
    pub periodic_damage: u16,
    pub base_armor: u16,
    pub raw_0x12: u8,
    pub flags_b: u8,
    pub dead: bool,
    pub temp_effect_timer: u16,
    pub temp_effect_id: u16,
    pub saved_dexterity: u16,
    pub saved_charisma: u16,
    pub saved_strength: u16,
    pub saved_endurance: u16,
}

impl PlayerSave {
    pub fn from_player(p: &PlayerCharacter) -> PlayerSave {
        PlayerSave {
            class: p.class,
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
            dexterity_base: p.dexterity_base,
            dexterity_current: p.dexterity_current,
            charisma_base: p.charisma_base,
            charisma_current: p.charisma_current,
            strength_base: p.strength_base,
            strength_current: p.strength_current,
            endurance_base: p.endurance_base,
            endurance_current: p.endurance_current,
            equipment: p.equipment,
            inventory: p.inventory,
            additional_equipment: p.additional_equipment,
            special_equipment: p.special_equipment,
            hp_internal: p.hp_internal,
            periodic_damage: p.periodic_damage,
            base_armor: p.base_armor,
            raw_0x12: p.raw_0x12,
            flags_b: p.flags_b,
            dead: p.dead,
            temp_effect_timer: p.temp_effect_timer,
            temp_effect_id: p.temp_effect_id,
            saved_dexterity: p.saved_dexterity,
            saved_charisma: p.saved_charisma,
            saved_strength: p.saved_strength,
            saved_endurance: p.saved_endurance,
        }
    }

    pub fn into_player(self, hero_index: u32) -> Option<PlayerCharacter> {
        let h = hero::hero(hero_index)?;
        Some(PlayerCharacter {
            hero: *h,
            class: self.class,
            level: self.level,
            money: self.money,
            available_xp: self.available_xp,
            invested_xp: self.invested_xp,
            melee_mastery: self.melee_mastery,
            trade: self.trade,
            archery_mastery: self.archery_mastery,
            building: self.building,
            healing: self.healing,
            identification: self.identification,
            smithing: self.smithing,
            dexterity_base: self.dexterity_base,
            dexterity_current: self.dexterity_current,
            charisma_base: self.charisma_base,
            charisma_current: self.charisma_current,
            strength_base: self.strength_base,
            strength_current: self.strength_current,
            endurance_base: self.endurance_base,
            endurance_current: self.endurance_current,
            equipment: self.equipment,
            inventory: self.inventory,
            additional_equipment: self.additional_equipment,
            special_equipment: self.special_equipment,
            hp_internal: self.hp_internal,
            periodic_damage: self.periodic_damage,
            base_armor: self.base_armor,
            raw_0x12: self.raw_0x12,
            flags_b: self.flags_b,
            dead: self.dead,
            dialogue_id: 0xFF,
            interface_mode_d2: 0,
            state_d3: 0,
            temp_effect_timer: self.temp_effect_timer,
            temp_effect_id: self.temp_effect_id,
            saved_dexterity: self.saved_dexterity,
            saved_charisma: self.saved_charisma,
            saved_strength: self.saved_strength,
            saved_endurance: self.saved_endurance,
        })
    }
}

/// One map's runtime population cache.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PopulationSave {
    pub loc_id: u32,
    pub build: PopulationBuild,
}

/// Complete persistent game state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SaveGame {
    pub meta: SaveMeta,
    pub player: PlayerSave,
    /// Player committed runtime cell and facing (`Entity+0x08/+0x0A/+0x01`).
    pub player_x: f64,
    pub player_y: f64,
    pub player_dir: i32,
    pub groups: Vec<Group>,
    pub entities: Vec<Entity>,
    pub settlements: Vec<Settlement>,
    pub settlement_trade: Vec<SettlementTrade>,
    pub world_object_sources: Vec<WorldObjectSource>,
    pub active_world_objects: Vec<ActiveWorldObject>,
    pub population: Vec<PopulationSave>,
    pub party: Vec<crate::actors::PartyMember>,
    pub party_selected: Vec<bool>,
    /// Full `B7` slots (300 x u32), including journal lines.
    pub quest_flags: Vec<u32>,
    pub discovered_locations: Vec<u8>,
    pub world_frame: u64,
    pub item_table: Vec<u8>,
    pub projectiles: Vec<Projectile>,
    pub rng: OriginalRng,
}

/// Runtime quest fields for one `Settlement` record. Kept outside `Settlement`
/// so the v1 bincode layout remains decodable; `Settlement` carries them as
/// `#[serde(skip)]` runtime fields.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct SettlementExtra {
    /// `+0x10` money pool (quest `0x11B`/`0x137`/`0x16D`).
    pub money_pool_10: i32,
    /// `+0x00` target-list base byte (`0x147`/`0x169`).
    pub target_base0: u8,
    /// `+0x01` target-list count byte (`0x169`).
    pub target_base1: u8,
}

/// Runtime quest fields for one `Group` record.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct GroupExtra {
    /// `+0x1D` group activity byte (`0x150`).
    pub activity_1d: u8,
    /// `DAT_0067C0A9[group*0x20] = 1` (`0x112`).
    pub visited: bool,
    /// `+0x02` i32 signed population-growth accumulator (`FUN_0041BD3C`).
    pub growth_timer: i32,
    /// `+0x06` i16 population-target countdown (`FUN_0041BD3C`).
    pub growth_countdown: i16,
}

/// Version-2 group extras (no growth fields). Used only to decode old payloads.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub(crate) struct GroupExtraV2 {
    pub(crate) activity_1d: u8,
    pub(crate) visited: bool,
}

/// Version-2 envelope extras: the same fields the shipped v2 build wrote.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct SaveExtraV2 {
    pub(crate) player_dialogue_id: u8,
    pub(crate) player_interface_mode_d2: u8,
    pub(crate) player_state_d3: u8,
    pub(crate) settlements: Vec<SettlementExtra>,
    pub(crate) groups: Vec<GroupExtraV2>,
}

/// Version-2 envelope. `SaveGame` did not change layout, so only the extras
/// differ.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct SaveEnvelopeV2 {
    pub(crate) game: SaveGame,
    pub(crate) extra: SaveExtraV2,
}

/// Version-3 envelope extras: the same fields the shipped v3 build wrote.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct SaveExtraV3 {
    pub(crate) player_dialogue_id: u8,
    pub(crate) player_interface_mode_d2: u8,
    pub(crate) player_state_d3: u8,
    pub(crate) settlements: Vec<SettlementExtra>,
    pub(crate) groups: Vec<GroupExtra>,
    pub(crate) leader_entity_index: u32,
}

/// Version-3 envelope. `SaveGame` did not change layout, so only the extras
/// differ.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct SaveEnvelopeV3 {
    pub(crate) game: SaveGame,
    pub(crate) extra: SaveExtraV3,
}

/// Version-4 envelope extras: the same fields the shipped v4 build wrote
/// (before the NPC `Entity+0xD2` map).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct SaveExtraV4 {
    pub(crate) player_dialogue_id: u8,
    pub(crate) player_interface_mode_d2: u8,
    pub(crate) player_state_d3: u8,
    pub(crate) settlements: Vec<SettlementExtra>,
    pub(crate) groups: Vec<GroupExtra>,
    pub(crate) apprentice_start: Vec<[i32; 3]>,
    pub(crate) leader_entity_index: u32,
}

/// Version-4 envelope. `SaveGame` did not change layout, so only the extras
/// differ.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct SaveEnvelopeV4 {
    pub(crate) game: SaveGame,
    pub(crate) extra: SaveExtraV4,
}

/// Quest/state additions introduced after v1. Serialized after the unchanged
/// `SaveGame`, so a v1 payload still decodes as a bare `SaveGame`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SaveExtra {
    pub player_dialogue_id: u8,
    pub player_interface_mode_d2: u8,
    pub player_state_d3: u8,
    pub settlements: Vec<SettlementExtra>,
    pub groups: Vec<GroupExtra>,
    /// Per-settlement apprentice training timestamps (`Settlement+0x3FC/400/404`).
    /// Empty means all zero.
    pub apprentice_start: Vec<[i32; 3]>,
    /// `Entity` index the protagonist was loaded from (legacy snapshot header).
    pub leader_entity_index: u32,
    /// Per-entity `Entity+0xD2` state bytes (2000 records, index = entity id),
    /// written by class-11 subtype 33 «Кукла». Empty in v4 and older payloads
    /// (the authored `Game.N` value is then restored on load).
    pub npc_state_d2: Vec<u8>,
}

impl Default for SaveExtra {
    fn default() -> Self {
        SaveExtra {
            // `Entity+0xD0 = 0xFF` is "no dialogue" for the protagonist.
            player_dialogue_id: 0xFF,
            player_interface_mode_d2: 0,
            player_state_d3: 0,
            settlements: Vec::new(),
            groups: Vec::new(),
            apprentice_start: Vec::new(),
            leader_entity_index: 0,
            npc_state_d2: Vec::new(),
        }
    }
}

/// Versioned save envelope. v2 = `SaveGame` + `SaveExtra`; v1 = bare `SaveGame`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SaveEnvelope {
    pub game: SaveGame,
    pub extra: SaveExtra,
}

impl SaveEnvelope {
    pub fn validate(&self) -> Result<(), String> {
        self.game.validate()?;
        if !self.extra.settlements.is_empty()
            && self.extra.settlements.len() != crate::population::SETTLEMENT_COUNT
        {
            return Err(format!(
                "settlement extras have {} records, expected {}",
                self.extra.settlements.len(),
                crate::population::SETTLEMENT_COUNT
            ));
        }
        if !self.extra.groups.is_empty()
            && self.extra.groups.len() != crate::population::GROUP_COUNT
        {
            return Err(format!(
                "group extras have {} records, expected {}",
                self.extra.groups.len(),
                crate::population::GROUP_COUNT
            ));
        }
        if !self.extra.apprentice_start.is_empty()
            && self.extra.apprentice_start.len() != crate::population::SETTLEMENT_COUNT
        {
            return Err(format!(
                "apprentice extras have {} records, expected {}",
                self.extra.apprentice_start.len(),
                crate::population::SETTLEMENT_COUNT
            ));
        }
        if !self.extra.npc_state_d2.is_empty()
            && self.extra.npc_state_d2.len() != crate::population::ENTITY_COUNT
        {
            return Err(format!(
                "npc state table has {} records, expected {}",
                self.extra.npc_state_d2.len(),
                crate::population::ENTITY_COUNT
            ));
        }
        Ok(())
    }

    /// Header + bincode + deflate. Returns bytes ready for base64/localStorage.
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let raw = bincode::serialize(self).map_err(|e| format!("save encode: {e}"))?;
        let mut enc =
            flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(&raw)
            .map_err(|e| format!("save compress: {e}"))?;
        let compressed = enc.finish().map_err(|e| format!("save compress: {e}"))?;
        let mut out = Vec::with_capacity(10 + compressed.len());
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.extend_from_slice(&(raw.len() as u32).to_le_bytes());
        out.extend_from_slice(&compressed);
        Ok(out)
    }

    /// Decode a v1..v5 payload. v1 payloads get default quest extras, v2
    /// payloads get default growth extras, v3 payloads get default apprentice
    /// timestamps and v4 payloads an empty NPC `+0xD2` map.
    pub fn decode(bytes: &[u8]) -> Result<SaveEnvelope, String> {
        if bytes.len() < 10 {
            return Err("save payload is truncated".into());
        }
        if bytes[0..4] != MAGIC {
            return Err("not an OpenKonung save payload".into());
        }
        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        if version != VERSION
            && version != VERSION_V4
            && version != VERSION_V3
            && version != VERSION_V2
            && version != VERSION_V1
        {
            return Err(format!("unsupported save version {version}"));
        }
        let raw_len = u32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]) as usize;
        if raw_len > 64 * 1024 * 1024 {
            return Err("save payload declares an unreasonable size".into());
        }
        let mut dec = flate2::read::DeflateDecoder::new(&bytes[10..]);
        let mut raw = Vec::with_capacity(raw_len);
        dec.read_to_end(&mut raw)
            .map_err(|e| format!("save decompress: {e}"))?;
        if raw.len() != raw_len {
            return Err(format!(
                "save payload length {} does not match declared {}",
                raw.len(),
                raw_len
            ));
        }
        let envelope = if version == VERSION {
            let env: SaveEnvelope =
                bincode::deserialize(&raw).map_err(|e| format!("save decode: {e}"))?;
            env
        } else if version == VERSION_V4 {
            let env: SaveEnvelopeV4 =
                bincode::deserialize(&raw).map_err(|e| format!("save decode v4: {e}"))?;
            SaveEnvelope {
                game: env.game,
                extra: SaveExtra {
                    player_dialogue_id: env.extra.player_dialogue_id,
                    player_interface_mode_d2: env.extra.player_interface_mode_d2,
                    player_state_d3: env.extra.player_state_d3,
                    settlements: env.extra.settlements,
                    groups: env.extra.groups,
                    apprentice_start: env.extra.apprentice_start,
                    leader_entity_index: env.extra.leader_entity_index,
                    npc_state_d2: Vec::new(),
                },
            }
        } else if version == VERSION_V3 {
            let env: SaveEnvelopeV3 =
                bincode::deserialize(&raw).map_err(|e| format!("save decode v3: {e}"))?;
            SaveEnvelope {
                game: env.game,
                extra: SaveExtra {
                    player_dialogue_id: env.extra.player_dialogue_id,
                    player_interface_mode_d2: env.extra.player_interface_mode_d2,
                    player_state_d3: env.extra.player_state_d3,
                    settlements: env.extra.settlements,
                    groups: env.extra.groups,
                    apprentice_start: Vec::new(),
                    leader_entity_index: env.extra.leader_entity_index,
                    npc_state_d2: Vec::new(),
                },
            }
        } else if version == VERSION_V2 {
            let env: SaveEnvelopeV2 =
                bincode::deserialize(&raw).map_err(|e| format!("save decode v2: {e}"))?;
            SaveEnvelope {
                game: env.game,
                extra: SaveExtra {
                    player_dialogue_id: env.extra.player_dialogue_id,
                    player_interface_mode_d2: env.extra.player_interface_mode_d2,
                    player_state_d3: env.extra.player_state_d3,
                    settlements: env.extra.settlements,
                    groups: env
                        .extra
                        .groups
                        .into_iter()
                        .map(|g| GroupExtra {
                            activity_1d: g.activity_1d,
                            visited: g.visited,
                            growth_timer: 0,
                            growth_countdown: 0,
                        })
                        .collect(),
                    apprentice_start: Vec::new(),
                    leader_entity_index: 0,
                    npc_state_d2: Vec::new(),
                },
            }
        } else {
            let game: SaveGame =
                bincode::deserialize(&raw).map_err(|e| format!("save decode v1: {e}"))?;
            SaveEnvelope {
                game,
                extra: SaveExtra::default(),
            }
        };
        envelope.validate()?;
        Ok(envelope)
    }
}

impl SaveGame {
    /// Structural validation before a save is applied to a live game.
    pub fn validate(&self) -> Result<(), String> {
        if self.meta.profile > 2 {
            return Err(format!("invalid Game profile {}", self.meta.profile));
        }
        if self.meta.hero_index > 2 {
            return Err(format!("invalid hero index {}", self.meta.hero_index));
        }
        if self.groups.len() != crate::population::GROUP_COUNT {
            return Err(format!(
                "group table has {} records, expected {}",
                self.groups.len(),
                crate::population::GROUP_COUNT
            ));
        }
        if self.entities.len() != crate::population::ENTITY_COUNT {
            return Err(format!(
                "entity table has {} records, expected {}",
                self.entities.len(),
                crate::population::ENTITY_COUNT
            ));
        }
        if self.settlements.len() != crate::population::SETTLEMENT_COUNT
            || self.settlement_trade.len() != crate::population::SETTLEMENT_COUNT
        {
            return Err(format!(
                "settlement tables have {}/{} records, expected {}",
                self.settlements.len(),
                self.settlement_trade.len(),
                crate::population::SETTLEMENT_COUNT
            ));
        }
        if self.item_table.len() != ITEM_TABLE_LEN {
            return Err(format!(
                "item table is {} bytes, expected {}",
                self.item_table.len(),
                ITEM_TABLE_LEN
            ));
        }
        if self.discovered_locations.len() != DISCOVERED_LEN {
            return Err(format!(
                "discovered mask has {} bytes, expected {}",
                self.discovered_locations.len(),
                DISCOVERED_LEN
            ));
        }
        if self.quest_flags.len() != FLAG_SLOTS {
            return Err(format!(
                "quest flags have {} slots, expected {}",
                self.quest_flags.len(),
                FLAG_SLOTS
            ));
        }
        for (i, e) in self.entities.iter().enumerate() {
            if e.index != i {
                return Err(format!("entity {i} carries index {}", e.index));
            }
        }
        for (i, g) in self.groups.iter().enumerate() {
            if g.index != i {
                return Err(format!("group {i} carries index {}", g.index));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_rejects_foreign_payloads() {
        assert!(SaveEnvelope::decode(b"nope").is_err());
        assert!(SaveEnvelope::decode(b"NOTASAVE0000000000").is_err());
    }
}
