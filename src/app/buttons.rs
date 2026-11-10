//! The original HUD button actions (`FUN_0042260C` left-button codes 1..=7).
//!
//! The seven-icon strip `0x81CF68[0..7]` is hit-tested by
//! `crate::hud::button_at` (the exact `FUN_00438900` row/run tables); a hit
//! resolves to one of the `crate::hud::CODE_*` commands, and this module
//! reproduces what each original handler does:
//!
//! | code | icon | original handler | effect |
//! |------|------|------------------|--------|
//! | 1 | weapon plaque | `FUN_004216DC(-1)` | cycle the active main weapon slot |
//! | 2 | stance | `FUN_00421554` | battle/peace for the selected units |
//! | 3 | map | `FUN_0042CBD0(4)` | world map (interface mode 5) |
//! | 4 | halt | `FUN_0042246C` | stop the selected units |
//! | 5 | engage | `FUN_00421458` | battle stance + attack nearest hostile |
//! | 6 | quest log | `FUN_004242BC` | mode 2 |
//! | 7 | character | `FUN_00424330` | mode 3 |
//!
//! Each press also flashes its icon (`0x81CF68[i] |= 0x80`) via `hud_press`.

use super::*;

impl PlayableApp {
    /// Dispatch one original HUD command. Returns `true` when handled.
    pub(super) fn press_hud_button(&mut self, code: u8) -> bool {
        self.hud_press = Some((code, 0.15));
        match code {
            crate::hud::CODE_WEAPON => {
                self.cycle_weapon_slots();
                true
            }
            crate::hud::CODE_STANCE => {
                if !self.mode_graph {
                    self.toggle_selected_battle_stance();
                }
                true
            }
            crate::hud::CODE_MAP => {
                self.toggle_map();
                true
            }
            crate::hud::CODE_HALT => {
                self.party_halt();
                true
            }
            crate::hud::CODE_ENGAGE => {
                self.party_engage();
                true
            }
            crate::hud::CODE_QUEST => {
                self.toggle_quest_log();
                true
            }
            crate::hud::CODE_CHARACTER => {
                self.toggle_character_screen();
                true
            }
            _ => false,
        }
    }

    /// `FUN_00424330` entry (same as the remake's `C` binding).
    pub(super) fn toggle_character_screen(&mut self) {
        if self.mode_graph {
            return;
        }
        self.queue_sound(crate::sound::events::UI_CLICK);
        self.character_screen = !self.character_screen;
        if self.character_screen {
            self.map_open = false;
            self.map_overlay = None;
            self.quest_log_open = false;
            self.sweep_all_owners();
        }
        self.hovered_slot = None;
    }

    /// `FUN_004216DC(-1)`: advance every selected unit to the next occupied
    /// main weapon slot (`Entity+0x66/+0x68/+0x6A`). Without Shift the original
    /// only touches the leader (loop bound 1); with Shift it scans up to 10
    /// selected units.
    fn cycle_weapon_slots(&mut self) {
        self.queue_sound(crate::sound::events::UI_CLICK);
        let mut changed = false;
        if self.party_selected.first().copied().unwrap_or(false) {
            let eq = self.player.equipment;
            let next = next_weapon_slot(self.player.raw_0x12, &eq);
            if next != self.player.raw_0x12 {
                self.player.raw_0x12 = next;
                changed = true;
            }
        }
        if self.shift_down {
            for (i, m) in self.party.iter_mut().enumerate() {
                if self.party_selected.get(i + 1).copied().unwrap_or(false) {
                    let next = next_weapon_slot(m.entity.raw_0x12, &m.entity.equipment);
                    if next != m.entity.raw_0x12 {
                        m.entity.raw_0x12 = next;
                        changed = true;
                    }
                }
            }
        }
        if changed {
            self.anim_time = 0.0;
        }
    }

    /// `FUN_0042246C`: stop selected units (`order 0x60`, clear route).
    pub(super) fn party_halt(&mut self) {
        self.queue_sound(crate::sound::events::UI_CLICK);
        self.party_halt_silent();
    }

    /// Halt without its own click sound (used by `party_engage`).
    fn party_halt_silent(&mut self) {
        if self.party_selected.first().copied().unwrap_or(false) {
            self.target = None;
            self.player_path.clear();
            self.player_active = false;
            self.player_step_t = 0.0;
            self.player_attack_target = None;
            self.player_attack_ranged = false;
        }
        for (i, m) in self.party.iter_mut().enumerate() {
            if self.party_selected.get(i + 1).copied().unwrap_or(false) {
                m.motion.path.clear();
                m.motion.target = None;
                m.motion.walking = false;
                m.motion.combat_target = None;
                m.motion.ranged_order = false;
            }
        }
    }

    /// `FUN_00421458`: put **every** party unit into battle stance and engage
    /// the nearest active hostile. The original only writes the order byte and
    /// clears the target words; the remake resolves a concrete target so the
    /// button has a visible effect and lets `A` start a fight without a manual
    /// right-click. Movement is not cancelled (the original does not halt).
    pub(super) fn party_engage(&mut self) {
        self.queue_sound(crate::sound::events::UI_CLICK);
        let any = !self.player.dead || self.party.iter().any(|m| !m.motion.dead);
        if !any {
            return;
        }
        self.set_all_party_battle();
        match self.nearest_engage_target() {
            Some(target) => {
                self.order_party_attack(target);
            }
            None => {
                self.combat_message = Some("Нет врагов поблизости".to_string());
                self.combat_message_t = 1.5;
            }
        }
    }

    /// Original `FUN_004216DC(n)` direct weapon-slot selection (keys 1/2/3,
    /// slots 2/0/1). Applies to the leader and selected companions.
    pub(super) fn select_weapon_slot(&mut self, slot: u8) {
        if slot > 2 {
            return;
        }
        self.queue_sound(crate::sound::events::UI_CLICK);
        let mut changed = false;
        if self.party_selected.first().copied().unwrap_or(false) {
            if self.player.raw_0x12 != slot {
                self.player.raw_0x12 = slot;
                changed = true;
            }
        }
        for (i, m) in self.party.iter_mut().enumerate() {
            if self.party_selected.get(i + 1).copied().unwrap_or(false) && m.entity.raw_0x12 != slot
            {
                m.entity.raw_0x12 = slot;
                changed = true;
            }
        }
        if changed {
            self.anim_time = 0.0;
        }
    }

    /// AUTO (`INTR[169/170]`, `Entity+0x03 & 2`): original `FUN_0041079C`
    /// auto-spends available XP on level/stat raises when the bit is set.
    pub(super) fn toggle_auto_xp(&mut self) {
        self.queue_sound(crate::sound::events::UI_CLICK);
        self.player.flags_b ^= 0x02;
        if self.player.auto_enabled() {
            self.auto_spend_xp(crate::inventory::InventoryOwner::Protagonist);
        }
    }

    /// Bounded port of the `FUN_0041079C` auto-raise loop (level row first,
    /// then Charisma/Strength/Dexterity/Endurance) for `owner`.
    pub(super) fn auto_spend_xp(&mut self, owner: crate::inventory::InventoryOwner) {
        use crate::character::PrimaryStat;
        for _ in 0..64 {
            if self.level_up_owner(owner) {
                continue;
            }
            let mut spent = false;
            for stat in [
                PrimaryStat::Charisma,
                PrimaryStat::Strength,
                PrimaryStat::Dexterity,
                PrimaryStat::Endurance,
            ] {
                if self.raise_owner_stat(owner, stat) {
                    spent = true;
                    break;
                }
            }
            if !spent {
                break;
            }
        }
    }

    /// Nearest live object/monster within the engage radius whose group is
    /// awake or that is already targeting the party.
    fn nearest_engage_target(&self) -> Option<crate::combat::CombatTarget> {
        const ENGAGE_RADIUS: i32 = 12;
        let loc = self.current_map;
        let player = (self.player_x.round() as i32, self.player_y.round() as i32);
        let pop = self.population_cache.get(&loc)?;
        let tables = self.tables.as_ref();
        let mut best: Option<(i32, usize)> = None;
        for a in pop.actors.iter() {
            if a.entity.archetype & 0x40 == 0 {
                continue;
            }
            let Some(m) = pop.motion.get(&a.entity.index) else {
                continue;
            };
            if m.dead {
                continue;
            }
            let awake = tables
                .and_then(|t| t.groups.get(a.source_group_id))
                .map(|g| g.activity_1d == 1)
                .unwrap_or(false);
            let targeting_party = matches!(
                m.combat_target,
                Some(crate::combat::CombatTarget::Player)
                    | Some(crate::combat::CombatTarget::Party(_))
            );
            if !awake && !targeting_party {
                continue;
            }
            let d = crate::actors::follow_distance(player, m.cell);
            if d > ENGAGE_RADIUS {
                continue;
            }
            if best.map(|(bd, _)| d < bd).unwrap_or(true) {
                best = Some((d, a.entity.index));
            }
        }
        best.map(|(_, id)| crate::combat::CombatTarget::World(id))
    }
}

/// Original occupied-slot cycle over the three main weapon slots.
fn next_weapon_slot(raw: u8, eq: &[u16; 6]) -> u8 {
    if eq[0] == 0 && eq[1] == 0 && eq[2] == 0 {
        return raw;
    }
    match raw {
        0 => {
            if eq[1] != 0 {
                1
            } else if eq[2] != 0 {
                2
            } else {
                0
            }
        }
        1 => {
            if eq[2] != 0 {
                2
            } else if eq[0] != 0 {
                0
            } else {
                1
            }
        }
        _ => {
            if eq[0] != 0 {
                0
            } else if eq[1] != 0 {
                1
            } else {
                2
            }
        }
    }
}
