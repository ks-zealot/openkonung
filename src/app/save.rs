//! Capture/apply of the native save payload for [`PlayableApp`].
//!
//! The saved state mirrors the original snapshot classes: quest flags (full
//! 300 x u32 B7 slots), item instances, groups, entities, settlements, world
//! objects/markers, party, discovered-map flags and the world tick. On load the
//! runtime is restored directly; like the original load mode `1`, no population
//! placement or WorldObject rebuild is performed.

use super::*;
use crate::inventory::InventoryOwner;
use crate::save::{
    PlayerSave, PopulationSave, SaveEnvelope, SaveGame, SaveMeta, DISCOVERED_LEN, FLAG_SLOTS,
    ITEM_TABLE_LEN,
};

impl PlayableApp {
    /// Default display name source for a save slot (current location).
    pub fn current_location_name(&self) -> String {
        let idx = self.current_map.saturating_sub(1) as usize;
        self.location_names.get(idx).cloned().unwrap_or_default()
    }

    /// Snapshot the live game into a serializable payload.
    pub fn capture_save(&self) -> SaveEnvelope {
        let groups = self
            .tables
            .as_ref()
            .map(|t| t.groups.clone())
            .unwrap_or_default();
        let entities = self
            .tables
            .as_ref()
            .map(|t| t.entities.clone())
            .unwrap_or_default();
        let mut item_table = vec![0u8; ITEM_TABLE_LEN];
        let n = self.world.res.save_game.len().min(ITEM_TABLE_LEN);
        item_table[..n].copy_from_slice(&self.world.res.save_game[..n]);
        let mut quest_flags = self.quest_flags.clone();
        quest_flags.resize(FLAG_SLOTS, 0);
        let mut settlements = self.settlements.clone();
        settlements.resize(crate::population::SETTLEMENT_COUNT, Default::default());
        settlements.truncate(crate::population::SETTLEMENT_COUNT);
        let mut settlement_trade = self.settlement_trade.clone();
        settlement_trade.resize(crate::population::SETTLEMENT_COUNT, Default::default());
        settlement_trade.truncate(crate::population::SETTLEMENT_COUNT);
        let mut party_selected = self.party_selected.to_vec();
        party_selected.resize(crate::hud::PARTY_CELLS, false);
        let extra = self.quest_extra();
        let game = SaveGame {
            meta: SaveMeta {
                profile: self.profile,
                hero_index: self.player.hero.hero_index,
                current_map: self.current_map,
            },
            player: PlayerSave::from_player(&self.player),
            player_x: self.player_x,
            player_y: self.player_y,
            player_dir: self.player_dir,
            groups,
            entities,
            settlements,
            settlement_trade,
            world_object_sources: self.world_object_sources.clone(),
            active_world_objects: self.active_world_objects.clone(),
            population: self
                .population_cache
                .iter()
                .map(|(loc_id, build)| PopulationSave {
                    loc_id: *loc_id,
                    build: build.clone(),
                })
                .collect(),
            party: self.party.clone(),
            party_selected,
            quest_flags,
            discovered_locations: self.discovered_locations.to_vec(),
            world_frame: self.world_frame,
            item_table,
            projectiles: self.projectiles.clone(),
            rng: self.combat_rng.clone(),
        };
        SaveEnvelope { game, extra }
    }

    /// Replace the live game with a validated save payload.
    pub fn apply_save(&mut self, envelope: SaveEnvelope) -> Result<(), String> {
        envelope.validate()?;
        let SaveEnvelope { game: save, extra } = envelope;
        if self.world.loc(save.meta.current_map).is_none() {
            return Err(format!(
                "save refers to unknown location {}",
                save.meta.current_map
            ));
        }
        let discovered = save.discovered_locations.clone();
        let player = save
            .player
            .into_player(save.meta.hero_index)
            .ok_or_else(|| format!("unknown hero index {}", save.meta.hero_index))?;

        self.profile = save.meta.profile;
        self.current_map = save.meta.current_map;
        self.player = player;
        self.player.dialogue_id = extra.player_dialogue_id;
        self.player.interface_mode_d2 = extra.player_interface_mode_d2;
        self.player.state_d3 = extra.player_state_d3;
        self.leader_entity_index = extra.leader_entity_index as usize;
        self.player_x = save.player_x;
        self.player_y = save.player_y;
        self.player_render_x = save.player_x;
        self.player_render_y = save.player_y;
        self.player_dir = save.player_dir & 7;

        self.tables = Some(GameTables {
            groups: save.groups,
            entities: save.entities,
        });
        self.settlements = save.settlements;
        // Waypoint `+1` (role dialogue id) is authored data and is `serde(skip)`ed
        // from the save payload, so restore it from the still-attached `Game.N`
        // bytes before `set_save_game` replaces the buffer with the item table.
        if let Ok(authored) = crate::population::parse_settlements(&self.world.res.save_game) {
            for (live, authored) in self.settlements.iter_mut().zip(authored) {
                for (lw, aw) in live.waypoints.iter_mut().zip(authored.waypoints) {
                    lw.dialogue_id = aw.dialogue_id;
                }
            }
        }
        let authored_groups = crate::population::parse_groups(&self.world.res.save_game).ok();
        if let Some(tables) = self.tables.as_mut() {
            for (i, g) in tables.groups.iter_mut().enumerate() {
                if let Some(authored) = authored_groups.as_ref().and_then(|v| v.get(i)) {
                    g.target_group = authored.target_group;
                }
            }
            for (i, ge) in extra.groups.iter().enumerate() {
                if let Some(g) = tables.groups.get_mut(i) {
                    g.activity_1d = ge.activity_1d;
                    g.visited = ge.visited;
                    g.growth_timer = ge.growth_timer;
                    g.growth_countdown = ge.growth_countdown;
                }
            }
        }
        for (i, se) in extra.settlements.iter().enumerate() {
            if let Some(s) = self.settlements.get_mut(i) {
                s.money_pool_10 = se.money_pool_10;
                s.target_base0 = se.target_base0;
                s.target_base1 = se.target_base1;
            }
        }
        for (i, t) in extra.apprentice_start.iter().enumerate() {
            if let Some(s) = self.settlements.get_mut(i) {
                s.apprentice_start = *t;
            }
        }
        self.settlement_trade = save.settlement_trade;
        self.world_object_sources = save.world_object_sources;
        self.active_world_objects = save.active_world_objects;
        self.population_cache = save
            .population
            .into_iter()
            .map(|p| (p.loc_id, p.build))
            .collect();
        // Runtime NPC `Entity+0xD2` values (class-11 subtype 33 «Кукла»). The
        // `Entity` payload itself is unchanged, so v1..v4 saves still decode;
        // for those the authored `Game.N` bytes are restored instead.
        let d2_values: Option<Vec<u8>> =
            if extra.npc_state_d2.len() == crate::population::ENTITY_COUNT {
                Some(extra.npc_state_d2.clone())
            } else {
                crate::population::parse_entities(&self.world.res.save_game)
                    .ok()
                    .map(|v| v.iter().map(|e| e.state_d2).collect())
            };
        if let Some(values) = d2_values {
            if let Some(tables) = self.tables.as_mut() {
                for (e, &v) in tables.entities.iter_mut().zip(values.iter()) {
                    e.state_d2 = v;
                }
            }
            let tables = self.tables.as_ref();
            for build in self.population_cache.values_mut() {
                for actor in build.actors.iter_mut() {
                    if let Some(e) = tables.and_then(|t| t.entities.get(actor.source_entity_id())) {
                        actor.entity.state_d2 = e.state_d2;
                    }
                }
            }
        }
        self.party = save.party;
        self.quest_flags = save.quest_flags;
        self.discovered_locations = [0; DISCOVERED_LEN];
        let n = discovered.len().min(DISCOVERED_LEN);
        self.discovered_locations[..n].copy_from_slice(&discovered[..n]);
        self.world_frame = save.world_frame;
        self.combat_rng = save.rng;
        self.projectiles = save.projectiles;
        self.world.res.set_save_game(&save.item_table);

        for (i, selected) in save
            .party_selected
            .into_iter()
            .take(crate::hud::PARTY_CELLS)
            .enumerate()
        {
            self.party_selected[i] = selected;
        }
        if !self.party_selected.iter().any(|&v| v) {
            self.party_selected[0] = true;
        }

        // Reset transient interaction/UI/combat state.
        self.selected = Some(self.current_map);
        self.target = None;
        self.player_path.clear();
        self.player_step_t = 0.0;
        self.player_active = false;
        self.anim_time = 0.0;
        self.teleport_cooldown = 0.0;
        self.world_frame_accum = 0.0;
        self.temp_effect_accum = 0.0;
        self.party_step_t = 0.0;
        self.player_interact = false;
        self.pickup_message = None;
        self.pickup_message_t = 0.0;
        self.trade_message = None;
        self.trade_message_t = 0.0;
        self.character_screen = false;
        self.dialog = None;
        self.trade = None;
        self.held_item = None;
        self.drag = false;
        self.dragging = false;
        self.drag_start = None;
        self.mouse_left_down = false;
        self.inventory_owner = InventoryOwner::Protagonist;
        self.inventory_scroll = 0;
        self.shift_down = false;
        self.ctrl_down = false;
        self.torch_reveal = false;
        self.dig_armed = false;
        self.hovered_slot = None;
        self.combat_step_t = 0.0;
        self.combat_frame = self.world_frame;
        self.player_attack_target = None;
        self.player_attack_frame = 0;
        self.player_attack_frames = 1;
        self.player_attack_ranged = false;
        self.player_ranged_frame = 0;
        self.player_ranged_frames = 0;
        self.player_ranged_fired = false;
        self.player_death_frames = 1;
        self.player_death_frame = 0;
        self.combat_message = None;
        self.combat_message_t = 0.0;
        self.combat_test_target = None;
        self.combat_test_prop = None;
        self.camera_follow = true;
        // The 56-byte discovered mask is restored verbatim from the snapshot
        // (`FUN_004246C0` block at `0xB076C`); loading never discovers extra
        // locations.
        self.map_overlay = None;

        if !self.population_cache.contains_key(&self.current_map) {
            self.ensure_population(self.current_map);
        }
        self.sweep_all_owners();
        self.apply_settlement_props();
        self.center_camera_on_player(true, 0.0);
        Ok(())
    }

    /// Export the live game as an original `KONUNG.SA0..9` payload
    /// (`0xB07A4` bytes).
    ///
    /// The `Game.N`-shaped blocks are copied from the loaded `Game.N` bytes and
    /// patched with every field the remake models, so bytes the remake does not
    /// interpret keep the original values. The projectile pool has no recovered
    /// field map, so the export carries an empty pool (count `0`).
    pub fn export_legacy_snapshot(&self) -> Vec<u8> {
        use crate::legacy_save::*;
        let env = self.capture_save();
        let game = env.game;
        let save = self.world.res.save_game.as_slice();
        let base = |off: usize, len: usize| -> Vec<u8> {
            if save.len() >= off + len {
                save[off..off + len].to_vec()
            } else {
                vec![0u8; len]
            }
        };
        let groups = patch_groups(
            base(crate::constants::game_layout::GROUP_OFF, GROUP_BLOCK_LEN),
            &game.groups,
        );
        // Party members and the protagonist live outside `tables.entities` in
        // the remake; patch their canonical records before writing the block.
        let mut entities = game.entities.clone();
        for m in &game.party {
            if let Some(e) = entities.get_mut(m.entity.index) {
                *e = m.entity.clone();
            }
        }
        if let Some(e) = entities.get(self.leader_entity_index).cloned() {
            let patched = entity_from_player_save(&e, &game.player);
            entities[self.leader_entity_index] = patched;
        }
        let entities = patch_entities(
            base(crate::constants::game_layout::ENTITY_OFF, ENTITY_BLOCK_LEN),
            &entities,
        );
        let world_objects = patch_world_objects(
            base(
                crate::constants::game_layout::WORLD_OBJECT_OFF,
                WORLD_OBJECT_BLOCK_LEN,
            ),
            &game.world_object_sources,
        );
        let settlements = patch_settlements(
            base(
                crate::constants::game_layout::SETTLEMENT_OFF,
                SETTLEMENT_BLOCK_LEN,
            ),
            &game.settlements,
            &game.settlement_trade,
        );
        let markers = build_markers(
            &world_objects,
            &game.world_object_sources,
            &game.active_world_objects,
        );
        let mut snap = LegacySnapshot::empty();
        snap.opt2 = 1;
        snap.opt3 = 1;
        snap.world_tick = game.world_frame as u32;
        snap.leader_index = self.leader_entity_index as u32;
        snap.projectile_count = 0;
        snap.marker_count = game.active_world_objects.len().min(200) as u32;
        snap.quest_flags = game.quest_flags;
        snap.item_table = game.item_table;
        snap.markers = markers;
        snap.groups = groups;
        snap.world_objects = world_objects;
        snap.entities = entities;
        snap.settlements = settlements;
        snap.discovered = game.discovered_locations;
        snap.write()
    }

    /// Load an original `KONUNG.SA0..9` snapshot (load mode `1`: no growth, no
    /// population re-placement).
    pub fn import_legacy_snapshot(&mut self, bytes: &[u8]) -> Result<(), String> {
        use crate::legacy_save::LegacySnapshot;
        let snap = LegacySnapshot::parse(bytes)?;
        let leader = snap
            .leader()
            .ok_or_else(|| format!("legacy leader index {} out of range", snap.leader_index))?;
        let class = crate::character::CharacterClass::from_entity_class(leader.archetype & 7)
            .ok_or_else(|| {
                format!(
                    "legacy leader entity {} is class {}",
                    snap.leader_index, leader.archetype
                )
            })?;
        let groups = snap.groups();
        let entities = snap.entities();
        let current_map = groups
            .get(leader.group_id as usize)
            .map(|g| g.loc_id as u32)
            .filter(|loc| *loc != 0 && self.world.loc(*loc).is_some())
            .ok_or_else(|| {
                format!(
                    "legacy leader group {} has no valid location",
                    leader.group_id
                )
            })?;
        let settlements = snap.settlements();
        let settlement_trade = snap.trades();
        let active_world_objects = snap.markers();
        let world_object_sources = crate::world_objects::parse_source_block(&snap.world_objects);
        let player = crate::save::PlayerSave {
            class,
            level: leader.stats.level,
            money: leader.stats.money,
            available_xp: leader.stats.available_xp,
            invested_xp: leader.stats.invested_xp,
            melee_mastery: leader.stats.melee_mastery,
            trade: leader.stats.trade,
            archery_mastery: leader.stats.archery_mastery,
            building: leader.stats.building,
            healing: leader.stats.healing,
            identification: leader.stats.identification,
            smithing: leader.stats.smithing,
            dexterity_base: leader.stats.dexterity_base,
            dexterity_current: leader.stats.dexterity_current,
            charisma_base: leader.stats.charisma_base,
            charisma_current: leader.stats.charisma_current,
            strength_base: leader.stats.strength_base,
            strength_current: leader.stats.strength_current,
            endurance_base: leader.stats.endurance_base,
            endurance_current: leader.stats.endurance_current,
            equipment: leader.equipment,
            inventory: leader.inventory,
            additional_equipment: leader.additional_equipment,
            special_equipment: leader.special_equipment,
            hp_internal: leader.stats.hp_internal,
            periodic_damage: leader.stats.periodic_damage,
            base_armor: leader.stats.base_armor,
            raw_0x12: leader.raw_0x12,
            flags_b: leader.flags_b,
            dead: false,
            temp_effect_timer: leader.stats.temp_effect_timer,
            temp_effect_id: leader.stats.temp_effect_id,
            saved_dexterity: leader.stats.saved_dexterity,
            saved_charisma: leader.stats.saved_charisma,
            saved_strength: leader.stats.saved_strength,
            saved_endurance: leader.stats.saved_endurance,
        };
        let hero_index = self.player.hero.hero_index;

        // Group 0 holds the party: every member except the leader becomes a
        // companion with a motion state at its authored cell.
        let party = groups
            .first()
            .map(|g| {
                let start = g.first_entity as usize;
                let end = start
                    .saturating_add(g.entity_count as usize)
                    .min(entities.len());
                entities[start..end]
                    .iter()
                    .filter(|e| e.index != snap.leader_index as usize)
                    .map(|e| crate::actors::PartyMember {
                        entity: e.clone(),
                        motion: crate::actors::ActorMotion {
                            cell: (e.x as i32, e.y as i32),
                            render_cell: (e.x as i32, e.y as i32),
                            facing: e.facing,
                            ..Default::default()
                        },
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let mut extra = self.quest_extra();
        extra.player_dialogue_id = leader.dialogue_id;
        extra.leader_entity_index = snap.leader_index;
        extra.settlements = settlements
            .iter()
            .map(|s| crate::save::SettlementExtra {
                money_pool_10: s.money_pool_10,
                target_base0: s.target_base0,
                target_base1: s.target_base1,
            })
            .collect();
        extra.groups = groups
            .iter()
            .map(|g| crate::save::GroupExtra {
                activity_1d: g.activity_1d,
                visited: g.visited,
                growth_timer: g.growth_timer,
                growth_countdown: g.growth_countdown,
            })
            .collect();
        extra.apprentice_start = settlements.iter().map(|s| s.apprentice_start).collect();

        let mut party_selected = vec![false; crate::hud::PARTY_CELLS];
        party_selected[0] = true;
        let game = crate::save::SaveGame {
            meta: crate::save::SaveMeta {
                profile: self.profile,
                hero_index,
                current_map,
            },
            player,
            player_x: leader.x as f64,
            player_y: leader.y as f64,
            player_dir: leader.facing as i32 & 7,
            groups: groups.clone(),
            entities: entities.clone(),
            settlements,
            settlement_trade,
            world_object_sources,
            active_world_objects,
            population: Vec::new(),
            party,
            party_selected,
            quest_flags: snap.quest_flags.clone(),
            discovered_locations: snap.discovered.clone(),
            world_frame: snap.world_tick as u64,
            item_table: snap.item_table.clone(),
            projectiles: Vec::new(),
            rng: self.combat_rng.clone(),
        };
        self.apply_save(crate::save::SaveEnvelope { game, extra })?;
        self.leader_entity_index = snap.leader_index as usize;
        // `apply_save` refreshes `target_group` from the authored Game.N table;
        // the snapshot's runtime target is authoritative for a legacy load.
        if let Some(tables) = self.tables.as_mut() {
            for (i, g) in groups.iter().enumerate() {
                if let Some(dst) = tables.groups.get_mut(i) {
                    dst.target_group = g.target_group;
                }
            }
        }
        // Original load mode 1 keeps the snapshot state and never re-places
        // actors: replace the growth-aware cache `apply_save` may have built.
        if let Some(tables) = self.tables.as_ref() {
            if let Some(loc) = self.world.loc(self.current_map) {
                let collision = loc.collision.clone();
                let build =
                    crate::actors::build_population(tables, self.current_map as u8, &collision);
                self.population_cache.insert(self.current_map, build);
            }
        }
        Ok(())
    }
}

/// Write the protagonist's live `PlayerCharacter` fields into its `Entity`
/// record (the original leader entity is mutated in place).
fn entity_from_player_save(
    base: &crate::population::Entity,
    p: &crate::save::PlayerSave,
) -> crate::population::Entity {
    let mut e = base.clone();
    e.archetype = p.class.entity_class();
    e.raw_0x12 = p.raw_0x12;
    e.flags_b = p.flags_b;
    e.equipment = p.equipment;
    e.inventory = p.inventory;
    e.additional_equipment = p.additional_equipment;
    e.special_equipment = p.special_equipment;
    e.stats.level = p.level;
    e.stats.money = p.money;
    e.stats.available_xp = p.available_xp;
    e.stats.invested_xp = p.invested_xp;
    e.stats.melee_mastery = p.melee_mastery;
    e.stats.trade = p.trade;
    e.stats.archery_mastery = p.archery_mastery;
    e.stats.building = p.building;
    e.stats.healing = p.healing;
    e.stats.identification = p.identification;
    e.stats.smithing = p.smithing;
    e.stats.dexterity_base = p.dexterity_base;
    e.stats.dexterity_current = p.dexterity_current;
    e.stats.charisma_base = p.charisma_base;
    e.stats.charisma_current = p.charisma_current;
    e.stats.strength_base = p.strength_base;
    e.stats.strength_current = p.strength_current;
    e.stats.endurance_base = p.endurance_base;
    e.stats.endurance_current = p.endurance_current;
    e.stats.hp_internal = p.hp_internal;
    e.stats.periodic_damage = p.periodic_damage;
    e.stats.base_armor = p.base_armor;
    e.stats.temp_effect_timer = p.temp_effect_timer;
    e.stats.temp_effect_id = p.temp_effect_id;
    e.stats.saved_dexterity = p.saved_dexterity;
    e.stats.saved_charisma = p.saved_charisma;
    e.stats.saved_strength = p.saved_strength;
    e.stats.saved_endurance = p.saved_endurance;
    e
}
