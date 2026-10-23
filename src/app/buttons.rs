//! The original HUD button actions (`FUN_0042260C` left-button codes 1..=7).

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
    fn party_halt(&mut self) {
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

    /// `FUN_00421458`: put every selected living unit into battle stance and
    /// set its engage order (`'D'`/`'A'` by weapon slot, queue cleared).
    fn party_engage(&mut self) {
        let any = self.party_selected.iter().any(|&s| s);
        if !any {
            return;
        }
        let first_battle = if self.party_selected.first().copied().unwrap_or(false) {
            crate::anim::battle_from_flags(self.player.flags_b)
        } else {
            self.party
                .iter()
                .enumerate()
                .find(|(i, _)| self.party_selected.get(i + 1).copied().unwrap_or(false))
                .map(|(_, m)| crate::anim::battle_from_flags(m.entity.flags_b))
                .unwrap_or(false)
        };
        if !first_battle {
            self.toggle_selected_battle_stance();
        }
        self.party_halt();
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
