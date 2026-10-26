//! Sound-event hooks for gameplay actions and ambience.
//!
//! The ids and tables live in [`crate::sound::events`]; this module only
//! decides *when* to queue them. All chance rolls use the cosmetic
//! [`PlayableApp::sound_rng`], never `combat_rng`, so sound placement cannot
//! perturb deterministic combat.
//!
//! Every request goes through [`PlayableApp::queue_sound`] /
//! [`PlayableApp::queue_sound_at`], which reproduce `FUN_0042A13C`: the volume
//! gate (`< -4000` drops the request), the pan/volume helpers `FUN_00438DC4` /
//! `FUN_00438E24` for positioned sounds, and the three secondary-buffer sample
//! rates (`22050/19050/25050`).

use super::*;

impl PlayableApp {
    /// Queue one clip at the original's default arguments (`volume = 0`,
    /// `pan = 0`) and roll the secondary-buffer rate exactly like
    /// `FUN_0042A13C`.
    pub(super) fn queue_sound(&mut self, id: u16) {
        let roll = self.sound_rng.next_u32();
        let rate = crate::sound::buffer_rate(id, roll);
        self.sounds.push(crate::sound::SoundEvent {
            id,
            volume: 0,
            pan: 0,
            rate,
        });
    }

    /// Queue one positioned clip (`FUN_00438E24` volume / `FUN_00438DC4` pan).
    /// The buffer-rate roll is skipped when the request is inaudible, matching
    /// the original's early return before selecting a buffer.
    pub(super) fn queue_sound_at(&mut self, id: u16, cell: (i32, i32)) {
        let player = (
            self.player_x.round() as i32,
            self.player_y.round() as i32,
        );
        let volume = crate::sound::source_volume(cell, player);
        if !crate::sound::audible(volume) {
            return;
        }
        let pan = crate::sound::source_pan(cell.1.max(0) as u16, player.1.max(0) as u16);
        let roll = self.sound_rng.next_u32();
        let rate = crate::sound::buffer_rate(id, roll);
        self.sounds.push(crate::sound::SoundEvent {
            id,
            volume,
            pan,
            rate,
        });
    }

    /// Queue a positioned clip when a cell is known, otherwise a centred one.
    pub(super) fn queue_sound_opt(&mut self, id: u16, cell: Option<(i32, i32)>) {
        match cell {
            Some(cell) => self.queue_sound_at(id, cell),
            None => self.queue_sound(id),
        }
    }

    /// Queue the action sound for an actor whose action row just changed
    /// (`FUN_0041435C`). `action` is `Entity+0x00`, `weapon_slot` is
    /// `Entity+0x12`; `leader` selects the human walk id.
    pub(super) fn play_actor_action(
        &mut self,
        archetype: u8,
        action: u8,
        weapon_slot: u8,
        leader: bool,
        cell: Option<(i32, i32)>,
    ) {
        let roll = self.sound_rng.next_u32() % 100;
        if let Some(id) = crate::sound::events::actor_action(
            archetype,
            action,
            weapon_slot,
            leader,
            roll,
        ) {
            self.queue_sound_opt(id, cell);
        }
    }

    /// Melee impact (`FUN_0041435C` action row `5`).
    pub(super) fn play_melee_impact(
        &mut self,
        archetype: u8,
        weapon_slot: u8,
        cell: Option<(i32, i32)>,
    ) {
        let id = if archetype & 0x40 != 0 {
            crate::sound::events::monster_action(archetype, 5, 0)
        } else {
            Some(crate::sound::events::human_melee_impact(weapon_slot))
        };
        if let Some(id) = id {
            self.queue_sound_opt(id, cell);
        }
    }

    /// Successful facing dodge (`FUN_0041c3cc` return `-2`).
    pub(super) fn play_dodge(&mut self, cell: Option<(i32, i32)>) {
        self.queue_sound_opt(crate::sound::events::DODGE, cell);
    }

    /// Original state-0 ambience (`0x4365d4`): 2% of gameplay frames play one
    /// clip from the current location's eight-clip bank.
    pub(super) fn tick_ambient_sound(&mut self) {
        if self.sound_rng.next_u32() % 100 <= 98 {
            return;
        }
        let slot = self.sound_rng.next_u32() % 8;
        self.queue_sound(crate::sound::events::ambient(self.current_map, slot));
    }
}
