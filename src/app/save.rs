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
        if let Some(tables) = self.tables.as_mut() {
            for (i, ge) in extra.groups.iter().enumerate() {
                if let Some(g) = tables.groups.get_mut(i) {
                    g.activity_1d = ge.activity_1d;
                    g.visited = ge.visited;
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
        self.settlement_trade = save.settlement_trade;
        self.world_object_sources = save.world_object_sources;
        self.active_world_objects = save.active_world_objects;
        self.population_cache = save
            .population
            .into_iter()
            .map(|p| (p.loc_id, p.build))
            .collect();
        self.party = save.party;
        self.quest_flags = save.quest_flags;
        self.discovered_locations = [0; DISCOVERED_LEN];
        let n = discovered.len().min(DISCOVERED_LEN);
        self.discovered_locations[..n].copy_from_slice(&discovered[..n]);
        self.world_frame = save.world_frame;
        self.combat_rng = save.rng;
        self.projectiles = save.projectiles;
        self.world.res.set_save_game(&save.item_table);

        for (i, selected) in save.party_selected.into_iter().take(crate::hud::PARTY_CELLS).enumerate()
        {
            self.party_selected[i] = selected;
        }
        if !self.party_selected.iter().any(|&v| v) {
            self.party_selected[0] = true;
        }

        // Reset transient interaction/UI/combat state.
        self.selected = Some(self.current_map);
        self.mode_graph = false;
        self.target = None;
        self.player_path.clear();
        self.player_step_t = 0.0;
        self.player_active = false;
        self.anim_time = 0.0;
        self.teleport_cooldown = 0.0;
        self.last_teleport = None;
        self.teleport_error = None;
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
        self.mark_discovered(self.current_map);

        if !self.population_cache.contains_key(&self.current_map) {
            self.ensure_population(self.current_map);
        }
        self.sweep_all_owners();
        self.center_camera_on_player(true, 0.0);
        Ok(())
    }
}
