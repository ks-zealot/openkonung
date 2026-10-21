//! `input` subsystem of [`crate::app`] (split from `app.rs`).

use super::*;

impl PlayableApp {
    /// Index of the party-portrait cell under the pointer (2x5 grid), if any.
    /// The original *selects* the member here (`FUN_00424D8C`) and never opens
    /// the character screen from it.
    pub(super) fn party_cell_at(&self, x: f64, y: f64) -> Option<usize> {
        let (mx, my) = self.window_to_virtual(x, y)?;
        let (mx, my) = (mx.round() as i32, my.round() as i32);
        let l = crate::hud::layout(VIRTUAL_W);
        (0..crate::hud::PARTY_CELLS)
            .find(|&i| l.party_cell(i).map(|r| r.contains(mx, my)).unwrap_or(false))
    }


    /// Party member selection (`FUN_00424D8C`, selection array `0x81A814`).
    ///
    /// Without the additive modifier (`_DAT_0081D624`, Shift) the whole 10-slot
    /// set is cleared and `cell` becomes the sole selection; with Shift held,
    /// `cell` is added to the existing set. Selecting another member (or an empty
    /// cell) therefore deselects the previous one.
    pub(super) fn select_party_member(&mut self, cell: usize) {
        if !self.shift_down {
            self.party_selected = [false; crate::hud::PARTY_CELLS];
        }
        if let Some(slot) = self.party_selected.get_mut(cell) {
            *slot = true;
        }
    }


    /// Update the additive-selection modifier (`Event.shiftKey`).
    pub fn set_shift(&mut self, down: bool) {
        self.shift_down = down;
    }


    pub fn pointer_down(&mut self, button: u32, x: f64, y: f64) {
        // The game-drawn cursor follows the pointer (`_DAT_0081D5C0/_D5C4`).
        if let Some(v) = self.window_to_virtual(x, y) {
            self.cursor_virtual = v;
        }
        // While a dialogue is open it consumes every click: option selection
        // only (left button). It is closed with Escape or by an end option.
        if self.dialog.is_some() {
            if button == 1 {
                if let Some((vx, vy)) = self.window_to_virtual(x, y) {
                    self.dialog_option_click(vx.round() as i32, vy.round() as i32);
                }
            }
            return;
        }
        // While the character screen is open it consumes every click: no world
        // targeting and no click-through. It is closed with Escape only.
        if self.character_screen {
            return;
        }
        // A left-click on a party-portrait cell selects that member
        // (`FUN_00424D8C`; additive with Shift) and is consumed. It must not
        // issue a world movement command. The character screen is not opened by
        // this click (original: opened by a command) — use `C` instead.
        if button == 1 && !self.mode_graph {
            if let Some(cell) = self.party_cell_at(x, y) {
                self.select_party_member(cell);
                return;
            }
        }
        if button == 1 {
            // Left button: select a graph node, or move the party. Left-click
            // never opens a dialogue — the original talks on the right button
            // (`FUN_0042260C`, `_DAT_0081D5B0 & 1` = UI commands + move only).
            if self.mode_graph {
                let v = self.window_to_virtual(x, y);
                if let Some((vx, vy)) = v {
                    let hit = self
                        .node_rects
                        .iter()
                        .find(|(_, r)| {
                            vx >= r.0 as f64
                                && vx < (r.0 + r.2) as f64
                                && vy >= r.1 as f64
                                && vy < (r.1 + r.3) as f64
                        })
                        .map(|(k, _)| *k);
                    if let Some(lid) = hit {
                        self.current_map = lid;
                        self.selected = Some(lid);
                        self.mode_graph = false;
                        self.target = None;
                        self.player_interact = false;
                        self.rebuild_world_objects();
                        self.fit_detail();
                    }
                }
            } else {
                self.world_move(x, y);
            }
            self.drag = false;
            self.last_mouse = (x, y);
        } else if button == 3 {
            // Right button: world interaction — open a talkable human NPC's
            // dialogue; otherwise move (original `FUN_0042260C` right-button
            // branch, `_DAT_0081D5B0 & 2`, command code 0).
            if !self.mode_graph {
                self.world_interact(x, y);
                self.drag = false;
                self.last_mouse = (x, y);
            }
        } else if button == 2 {
            self.camera_follow = false;
            self.drag = true;
            self.last_mouse = (x, y);
        }
    }


    /// A left-click in the world: command the selected party to the clicked
    /// cell. Left-click never opens a dialogue — the original opens dialogues on
    /// the right button (`FUN_0042260C`). Matching the original command 0, the
    /// clicked cell is stored on **every** selected unit (selection array
    /// `0x81A814`): the leader and each selected companion path to the same cell.
    fn world_move(&mut self, x: f64, y: f64) {
        if self.player.dead {
            return;
        }
        // Left-click move clears `Entity+0x03` bit `0x40` on every selected
        // unit; ordinary walk-over therefore never collects. A new move order
        // also replaces any pending melee target.
        self.player_interact = false;
        self.player_attack_target = None;
        for m in self.party.iter_mut() {
            m.motion.combat_target = None;
        }
        self.world_move_command(x, y);
    }

    fn world_move_command(&mut self, x: f64, y: f64) {
        let Some((tx, ty)) = self.mouse_to_cell(x, y) else {
            return;
        };
        let interact = self.player_interact;
        if self.party_selected[0] {
            self.set_player_target(tx, ty);
        }
        self.command_selected_companions(tx, ty);
        // Right-click on the cell already occupied: no walk is issued, but the
        // interaction still resolves immediately.
        if interact
            && (self.player_x.round() as i32, self.player_y.round() as i32) == (tx, ty)
        {
            self.try_pickup_here();
            self.player_interact = false;
        }
    }


    /// A right-click in the world (original `FUN_0042260C` right-button branch,
    /// `_DAT_0081D5B0 & 2`, command code 0): open a dialogue for a **human**
    /// actor (`archetype & 0x40 == 0`) whose `dialogue_id != 0xFF`; otherwise
    /// issue a melee attack order to the selected units that are already in
    /// battle stance. If no unit is in battle stance, nothing is ordered and no
    /// move is issued (matching the original loop / early return).
    fn world_interact(&mut self, x: f64, y: f64) {
        if self.player.dead {
            return;
        }
        let Some((vx, vy)) = self.window_to_virtual(x, y) else {
            return;
        };
        let (vx, vy) = (vx.round() as i32, vy.round() as i32);
        if let Some((entity, archetype, dialogue_id)) = self.actor_at(vx, vy) {
            // Human visual family (`archetype & 0x40 == 0`) with a dialogue id
            // and the leader not in battle stance (`FUN_0042260C` dialogue gate).
            if archetype & 0x40 == 0
                && dialogue_id != 0xFF
                && !crate::anim::battle_from_flags(self.player.flags_b)
            {
                self.open_dialog(entity, dialogue_id);
                return;
            }
            // Same-group actor (recruited companion): do not order an attack.
            if self.party.iter().any(|m| m.entity.index == entity) {
                return;
            }
            // Non-dialogue actor (monster/object/non-talkable human).
            self.issue_attack_order(entity);
            return;
        }
        // Right-click empty ground sets the interaction/occupy flag and moves.
        self.player_interact = true;
        self.world_move_command(x, y);
    }


    /// Select the option under the cursor (if any) and advance the dialogue.
    fn dialog_option_click(&mut self, vx: i32, vy: i32) {
        let Some(index) = self.dialog.as_ref().and_then(|d| d.option_at(vx, vy)) else {
            return;
        };
        self.dialog_click_option(index);
    }


    pub fn pointer_up(&mut self, button: u32) {
        if button == 2 {
            self.drag = false;
        }
    }


    pub fn pointer_move(&mut self, x: f64, y: f64) {
        // The game-drawn cursor follows the pointer (`_DAT_0081D5C0/_D5C4`).
        if let Some(v) = self.window_to_virtual(x, y) {
            self.cursor_virtual = v;
        }
        // Dialogue hover: highlight the option under the cursor.
        if self.dialog.is_some() {
            if let Some((vx, vy)) = self.window_to_virtual(x, y) {
                let (vx, vy) = (vx.round() as i32, vy.round() as i32);
                if let Some(d) = self.dialog.as_mut() {
                    d.selected = d.option_at(vx, vy);
                }
            }
            return;
        }
        if self.drag {
            let dx = x - self.last_mouse.0;
            let dy = y - self.last_mouse.1;
            self.last_mouse = (x, y);
            self.hovered_slot = None;
            if self.mode_graph {
                self.graph_pan[0] += dx;
                self.graph_pan[1] += dy;
            } else {
                let (_, _, _, _, sc) = self.window_game_rect();
                self.detail_pan[0] += dx / sc.max(1e-6);
                self.detail_pan[1] += dy / sc.max(1e-6);
            }
            return;
        }
        if self.mode_graph {
            self.hovered_slot = None;
        } else {
            self.update_hover(x, y);
        }
    }


    /// Read-only HUD hover: which visible inventory cell is under the cursor.
    /// Does not touch world targeting, movement or portals.
    pub(super) fn update_hover(&mut self, x: f64, y: f64) {
        let Some((mx, my)) = self.window_to_virtual(x, y) else {
            self.hovered_slot = None;
            return;
        };
        let l = crate::hud::layout(VIRTUAL_W);
        let (mx, my) = (mx.round() as i32, my.round() as i32);
        self.hovered_slot = (0..l.visible_slots).find(|&slot| {
            l.inventory_slot_rect(slot, 70)
                .map(|r| r.contains(mx, my))
                .unwrap_or(false)
        });
    }


    pub fn wheel(&mut self, delta: f64) {
        if self.mode_graph {
            let factor = 1.2f64.powf(delta);
            self.graph_zoom = (self.graph_zoom * factor).clamp(0.04, 3.0);
        }
    }


    pub fn key_down(&mut self, code: u32) {
        // Dialogue captures the keyboard: only Escape closes it.
        if self.dialog.is_some() {
            if code == KEY_ESCAPE {
                self.dialog = None;
                self.hovered_slot = None;
            }
            return;
        }
        match code {
            // Escape closes the character screen when open; otherwise it opens
            // the frontend/menu (the live game is preserved by the host runtime).
            KEY_ESCAPE => {
                if self.character_screen {
                    self.character_screen = false;
                    self.hovered_slot = None;
                } else {
                    self.request_menu = true;
                }
            }
            KEY_H => {
                if !self.mode_graph {
                    self.mode_graph = true;
                    self.selected = None;
                } else {
                    self.mode_graph = false;
                    self.selected = Some(self.current_map);
                    self.fit_detail();
                }
            }
            KEY_BACKSPACE => {
                if !self.mode_graph {
                    self.mode_graph = true;
                    self.selected = None;
                }
            }
            // Tab runs the original SPACE stance command (`FUN_00421554`) over
            // the current party selection: every selected, living unit enters
            // or leaves battle according to the first selected unit's stance.
            KEY_TAB => {
                if !self.mode_graph {
                    self.toggle_selected_battle_stance();
                }
            }
            KEY_D => {
                self.debug_ids = !self.debug_ids;
            }
            KEY_P => {
                self.population_debug = !self.population_debug;
            }
            KEY_L => {
                self.layer_debug = !self.layer_debug;
            }
            KEY_T => {
                self.teleport_debug = !self.teleport_debug;
            }
            KEY_F => {
                if !self.mode_graph {
                    self.fit_detail();
                    if self.camera_follow {
                        self.center_camera_on_player(true, 0.0);
                    }
                }
            }
            // `C` toggles the character screen (the original opens it via a UI
            // command/action, not a mouse portrait click; this is the remake's
            // explicit binding). Escape also closes it.
            KEY_C => {
                if !self.mode_graph {
                    self.character_screen = !self.character_screen;
                    self.hovered_slot = None;
                }
            }
            // `V` toggles camera follow (moved off `C`).
            KEY_V => {
                if !self.mode_graph {
                    self.camera_follow = !self.camera_follow;
                    if self.camera_follow {
                        self.center_camera_on_player(true, 0.0);
                    }
                }
            }
            KEY_R => {
                self.world.clear_caches();
            }
            _ => {}
        }
    }

}
