//! Lower-panel hover text (`FUN_0042260C` hover branch).
//!
//! The original draws one left-aligned, word-wrapped line stack in the bottom
//! panel (`FUN_0042BD8C`) whose content is selected in this order:
//!
//! 1. a ground marker with non-negative money → `0x45C31C`
//!    «Что-то лежит на земле»;
//! 2. the actor under the cursor → `FUN_00430958`;
//! 3. the HUD hit-test (`FUN_004381C8`):
//!    - the seven-icon strip → fixed button descriptions (`FUN_0042E9CC`);
//!    - an inventory/equipment slot → `FUN_0042F5F8` item detail.
//!
//! The actor panel is the originally-unlocated `FUN_00430958`. Its name/class
//! tables are recovered in [`crate::names`]; the party/enemy/profession fields
//! are read from the same `Entity` state the rest of the remake uses.

use super::*;
use crate::population::Entity;

/// Fields `FUN_00430958` reads from the hovered actor.
pub(super) struct ActorPanelInfo<'a> {
    /// Display name: `0x45E71C[+0xD4]` plus optional `" " + 0x45FC7C[+0xD5]`
    /// (humans), or the monster table `0x45DB3C[archetype & 0x7F]`.
    pub name: &'a str,
    /// Class name (`0x45ADB0[archetype & 7]`), humans only.
    pub class: Option<&'a str>,
    pub archetype: u8,
    /// Same group as the leader (`_DAT_0081D55C`, group 0 in the remake).
    pub same_group_as_leader: bool,
    /// `Group+0x1D`; the original tests bit 1 for the `", враг"` suffix.
    pub activity: u8,
    /// `FUN_0041182C` role name (`староста`/`знахарь`/`купец`/`кузнец`/`воевода`).
    pub profession: Option<&'a str>,
    pub level: u8,
    /// `Entity+0x5C` internal health (`Здор = (hp + 15) >> 4`).
    pub health: u16,
    pub poisoned: bool,
    pub charisma: u16,
    pub strength: u16,
    pub dexterity: u16,
    pub endurance: u16,
    pub healing: u16,
    pub smithing: u16,
    pub trade: u16,
    pub building: u16,
    pub identification: u16,
}

/// Compose the actor panel exactly as `FUN_00430958` (`0x430958`).
pub(super) fn actor_panel_text(info: &ActorPanelInfo) -> String {
    let low = info.archetype & 0x7F;
    let mut out = info.name.to_string();
    if info.archetype & 0x40 == 0 {
        // Human: name, class, level/health, then loyalty/profession/skills.
        if let Some(class) = info.class {
            out.push_str(", ");
            out.push_str(class);
        }
        push_level_health(&mut out, info.level, info.health);
        if info.same_group_as_leader {
            out.push_str(", воин дружины");
            if info.poisoned {
                out.push_str(", отравлен");
            }
            out.push_str(" Хар:");
            out.push_str(&info.charisma.to_string());
            out.push_str(" Сил:");
            out.push_str(&info.strength.to_string());
            out.push_str(" Лов:");
            out.push_str(&info.dexterity.to_string());
            out.push_str(" Вын:");
            out.push_str(&info.endurance.to_string());
        } else if info.activity & 1 != 0 {
            out.push_str(", враг");
        } else if let Some(profession) = info.profession {
            out.push_str(", ");
            out.push_str(profession);
        } else {
            if info.healing > 100 {
                out.push_str(", знаком со знахарским делом");
            }
            if info.smithing > 4 {
                out.push_str(", знает кузнечное дело");
            }
            if info.trade > 0x1E {
                out.push_str(", опытен в торговле");
            }
            if info.building > 4 {
                out.push_str(", искусный строитель");
            }
            if info.identification > 10 {
                out.push_str(", разбирается в диковинных вещах");
            }
        }
    } else if low < 0x51 {
        // Monster: `FUN_0042BBC4` appends level/health too.
        push_level_health(&mut out, info.level, info.health);
    }
    out
}

/// Append `, Уров:N, Здор:M` in the original order.
///
/// Health is the internal `Entity+0x5C` value converted to displayed points by
/// `(hp + 15) >> 4` (so 1600 internal = the 100 shown on the character screen).
fn push_level_health(out: &mut String, level: u8, health: u16) {
    out.push_str(", Уров:");
    out.push_str(&level.to_string());
    out.push_str(", Здор:");
    out.push_str(&((health as u32 + 15) >> 4).to_string());
}

/// Fixed HUD button descriptions from `FUN_002260C` / `FUN_0042E9CC`.
fn hud_button_tooltip(code: u8) -> Option<&'static str> {
    Some(match code {
        crate::hud::CODE_WEAPON => "Сменить оружие (1, 2, 3)",
        crate::hud::CODE_STANCE => "Достать/Убрать оружие (Пробел)",
        crate::hud::CODE_MAP => "Карта (M)",
        crate::hud::CODE_HALT => "Все ко мне (F)",
        crate::hud::CODE_ENGAGE => "Атаковать (A)",
        crate::hud::CODE_QUEST => "Информация к размышлению (Q)",
        crate::hud::CODE_CHARACTER => "Панель персонажа (I)",
        _ => return None,
    })
}

impl PlayableApp {
    /// Text the original would draw in the bottom panel for the current pointer.
    ///
    /// `None` outside interface mode 0 (world) or when nothing is hovered.
    pub(super) fn hover_panel_text(&mut self) -> Option<String> {
        if self.mode_graph || self.drag || crate::video::is_pending() {
            return None;
        }
        // `_DAT_0081D610 == 0`: world mode only. Dialog/trade/windows keep their
        // own text and the original hover branch runs only for the world.
        if self.dialog.is_some()
            || self.character_screen
            || self.map_open
            || self.quest_log_open
            || self.trade.is_some()
        {
            return None;
        }
        let (cx, cy) = self.cursor_virtual;
        if cx < 0.0 || cy < 0.0 || cx >= VIRTUAL_W as f64 || cy >= VIRTUAL_H as f64 {
            return None;
        }
        let vx = cx.round() as i32;
        let vy = cy.round() as i32;
        let layout = crate::hud::layout(VIRTUAL_W);

        // The world hit test only runs over the world viewport (not the party
        // panel, inventory strip or bottom bar).
        if layout.viewport.contains(vx, vy) && !layout.inventory_region.contains(vx, vy) {
            let lx = vx - self.detail_pan[0].round() as i32;
            let ly = vy - self.detail_pan[1].round() as i32;
            let (tx, ty) = local_px_to_entity(lx, ly);
            // `DAT_004EE6B0[...] & 0x20` + object `+0x0E >> 16 >= 0`: active
            // ground marker with non-negative money (buried records excluded).
            if self
                .active_world_objects
                .iter()
                .any(|o| o.cell() == (tx, ty) && o.money >= 0)
            {
                return Some("Что-то лежит на земле".to_string());
            }
            if let Some(text) = self.actor_panel_at(vx, vy) {
                if !text.is_empty() {
                    return Some(text);
                }
            }
        }

        // Party portrait hover (`FUN_004381C8` code `0x1E`): the original also
        // calls `FUN_00430958` on the selected member; cell 0 is the leader.
        if let Some(cell) =
            (0..layout.party_cells.len()).find(|&i| layout.party_cells[i].contains(vx, vy))
        {
            if cell == 0 {
                if !self.player.dead {
                    return Some(self.player_panel_text());
                }
            } else if let Some(member) = self.party.get(cell - 1) {
                if !member.motion.dead && !matches!(member.motion.action, 3 | 9) {
                    return Some(self.entity_panel_text(&member.entity));
                }
            }
        }

        // Fixed HUD button descriptions (seven-icon lower-left strip).
        if let Some(code) = crate::hud::button_at(vx, vy, layout.logical_w) {
            if let Some(text) = hud_button_tooltip(code) {
                return Some(text.to_string());
            }
        }

        self.hovered_item_text()
    }

    /// Hovered world actor panel: leader first (like `cursor_actor_at`), then
    /// the sprite-accurate `actor_at` hit.
    fn actor_panel_at(&mut self, vx: i32, vy: i32) -> Option<String> {
        if self.leader_sprite_covers(vx, vy) {
            return Some(self.player_panel_text());
        }
        let (id, _, _, _) = self.actor_at(vx, vy)?;
        let entity = self.entity_by_index(id)?;
        Some(self.entity_panel_text(entity))
    }

    /// Canonical `Game.N` entity for `id`, falling back to the party overlay
    /// (recruited members shadow their source row).
    fn entity_by_index(&self, index: usize) -> Option<&Entity> {
        if let Some(pop) = self.population_cache.get(&self.current_map) {
            if let Some(actor) = pop.actors.iter().find(|a| a.source_entity_id() == index) {
                return Some(&actor.entity);
            }
        }
        self.party
            .iter()
            .find(|m| m.entity.index == index)
            .map(|m| &m.entity)
    }

    /// Compose `Entity+0xD4` name plus the optional `Entity+0xD5` nickname
    /// (`0x45E71C` / `0x45FC7C` tables, CP866).
    ///
    /// `nickname_id == 0` means "no nickname" and adds nothing; a missing table
    /// entry degrades to the empty string rather than a placeholder.
    fn character_display_name(&self, name_id: u8, nickname_id: u8) -> String {
        let mut name = self.world.res.character_name(name_id).unwrap_or_default();
        if nickname_id != 0 {
            if let Some(nickname) = self.world.res.nickname(nickname_id) {
                if !name.is_empty() {
                    name.push(' ');
                }
                name.push_str(&nickname);
            }
        }
        name
    }

    /// Resolve every [`ActorPanelInfo`] field for a world actor and compose the
    /// panel line.
    ///
    /// Name family is `FUN_0042BC60`'s rule: humans (`archetype & 0x40 == 0`)
    /// and non-humans above `0x50` use the character name table, monsters
    /// `0x41..=0x50` use the monster table indexed by `archetype & 0x7F`. The
    /// profession only applies to a settled human of another group whose group
    /// activity bit 1 is clear, i.e. `FUN_0041182C`'s role waypoint.
    fn entity_panel_text(&self, entity: &Entity) -> String {
        let low = entity.archetype & 0x7F;
        let human = entity.archetype & 0x40 == 0;
        // `FUN_0042BC60`: humans (and non-humans above 0x50) use the character
        // name table; monsters 0x41..=0x50 use the monster table.
        let name = if human || low > 0x50 {
            self.character_display_name(entity.name_id, entity.nickname_id)
        } else {
            self.world.res.monster_name(low).unwrap_or_default()
        };
        let class = if human {
            self.world.res.class_name(entity.archetype & 7)
        } else {
            None
        };
        let same_group = entity.group_id == 0;
        let activity = self
            .tables
            .as_ref()
            .and_then(|t| t.groups.get(entity.group_id as usize))
            .map(|g| g.activity_1d)
            .unwrap_or(0);
        let profession = if human && !same_group && activity & 1 == 0 {
            self.settlements
                .iter()
                .find(|s| s.loc_id as u32 == self.current_map)
                .and_then(|s| {
                    let role = s.role_of(entity.index as u16);
                    if role == 0 {
                        None
                    } else {
                        self.world.res.profession_name(role)
                    }
                })
        } else {
            None
        };
        let info = ActorPanelInfo {
            name: &name,
            class: class.as_deref(),
            archetype: entity.archetype,
            same_group_as_leader: same_group,
            activity,
            profession: profession.as_deref(),
            level: entity.stats.level,
            health: entity.stats.hp_internal,
            poisoned: entity.stats.periodic_damage != 0,
            charisma: entity.stats.charisma_base,
            strength: entity.stats.strength_base,
            dexterity: entity.stats.dexterity_base,
            endurance: entity.stats.endurance_base,
            healing: entity.stats.healing,
            smithing: entity.stats.smithing,
            trade: entity.stats.trade,
            building: entity.stats.building,
            identification: entity.stats.identification,
        };
        actor_panel_text(&info)
    }

    /// Panel line for the protagonist (party slot 0), always rendered as a
    /// same-group human so it carries the `, воин дружины` suffix and the four
    /// base stats.
    fn player_panel_text(&self) -> String {
        let name = self.character_display_name(self.player.hero.name_id, 0);
        let class = self.world.res.class_name(self.player.entity_class());
        let info = ActorPanelInfo {
            name: &name,
            class: class.as_deref(),
            archetype: self.player.entity_class(),
            same_group_as_leader: true,
            activity: 0,
            profession: None,
            level: self.player.level,
            health: self.player.hp_internal,
            poisoned: self.player.periodic_damage != 0,
            charisma: self.player.charisma_base,
            strength: self.player.strength_base,
            dexterity: self.player.dexterity_base,
            endurance: self.player.endurance_base,
            healing: self.player.healing,
            smithing: self.player.smithing,
            trade: self.player.trade,
            building: self.player.building,
            identification: self.player.identification,
        };
        actor_panel_text(&info)
    }

    /// Diagnostic wrapper for the current lower-panel hover text.
    pub fn debug_hover_text(&mut self) -> Option<String> {
        self.hover_panel_text()
    }

    /// Hovered visible inventory slot -> `FUN_0042F5F8` item detail text.
    ///
    /// Uses the current interface owner's 32-slot inventory offset by
    /// `inventory_scroll` (`_81D708`); an empty or out-of-range slot yields
    /// `None` so the panel keeps whatever the other branches produced.
    fn hovered_item_text(&self) -> Option<String> {
        let slot = self.hovered_slot?;
        let layout = crate::hud::layout(VIRTUAL_W);
        if slot >= layout.visible_slots {
            return None;
        }
        let index = self.inventory_scroll + slot;
        let inventory = self.owner_inventory(self.inventory_owner);
        let id = *inventory.get(index)?;
        if id == 0 {
            return None;
        }
        self.world.res.item_hover_detail(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Baseline human panel: no party membership, no profession, no skills.
    fn base_info() -> ActorPanelInfo<'static> {
        ActorPanelInfo {
            name: "Белун",
            class: Some("Воин"),
            archetype: 5,
            same_group_as_leader: false,
            activity: 0,
            profession: None,
            level: 3,
            health: 1600,
            poisoned: false,
            charisma: 12,
            strength: 15,
            dexterity: 7,
            endurance: 10,
            healing: 0,
            smithing: 0,
            trade: 0,
            building: 0,
            identification: 0,
        }
    }

    /// A human actor with no party/enemy/skill state prints exactly
    /// `Name, Class, Уров:N, Здор:M`.
    #[test]
    fn human_panel_shows_name_class_level_health() {
        assert_eq!(
            actor_panel_text(&base_info()),
            "Белун, Воин, Уров:3, Здор:100"
        );
    }

    /// A party member appends `, воин дружины`, the poison marker and the four
    /// base stats, in that order.
    #[test]
    fn party_panel_appends_loyalty_poison_and_stats() {
        let mut info = base_info();
        info.same_group_as_leader = true;
        info.poisoned = true;
        assert_eq!(
            actor_panel_text(&info),
            "Белун, Воин, Уров:3, Здор:100, воин дружины, отравлен Хар:12 Сил:15 Лов:7 Вын:10"
        );
    }

    /// An active hostility group wins over both profession and skill suffixes.
    #[test]
    fn active_group_is_enemy_before_profession_and_skills() {
        let mut info = base_info();
        info.activity = 1;
        info.profession = Some("купец");
        assert_eq!(
            actor_panel_text(&info),
            "Белун, Воин, Уров:3, Здор:100, враг"
        );
    }

    /// A profession name suppresses the skill suffixes; without one the five
    /// skill thresholds (`>100`, `>4`, `>0x1E`, `>4`, `>10`) are appended.
    #[test]
    fn profession_and_skills_use_the_original_thresholds() {
        let mut info = base_info();
        info.profession = Some("знахарь");
        assert_eq!(
            actor_panel_text(&info),
            "Белун, Воин, Уров:3, Здор:100, знахарь"
        );
        info.profession = None;
        info.healing = 101;
        info.smithing = 5;
        info.trade = 31;
        info.building = 5;
        info.identification = 11;
        assert_eq!(
            actor_panel_text(&info),
            "Белун, Воин, Уров:3, Здор:100, знаком со знахарским делом, знает кузнечное дело, опытен в торговле, искусный строитель, разбирается в диковинных вещах"
        );
    }

    /// A monster (`archetype & 0x40 != 0`, low byte `<= 0x50`) gets no class,
    /// no party/skill suffixes and its health divided by 16.
    #[test]
    fn monster_panel_shows_name_level_and_health() {
        let info = ActorPanelInfo {
            name: "Аспид",
            class: None,
            archetype: 0x42,
            same_group_as_leader: false,
            activity: 0,
            profession: None,
            level: 2,
            health: 320,
            poisoned: false,
            charisma: 0,
            strength: 0,
            dexterity: 0,
            endurance: 0,
            healing: 0,
            smithing: 0,
            trade: 0,
            building: 0,
            identification: 0,
        };
        assert_eq!(actor_panel_text(&info), "Аспид, Уров:2, Здор:20");
    }
}
