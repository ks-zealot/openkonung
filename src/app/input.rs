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

    /// Update the force-talk modifier (`Event.ctrlKey`, original `_DAT_0081D5E4`).
    pub fn set_ctrl(&mut self, down: bool) {
        self.ctrl_down = down;
    }

    pub fn pointer_down(&mut self, button: u32, x: f64, y: f64) {
        // The game-drawn cursor follows the pointer (`_DAT_0081D5C0/_D5C4`).
        if let Some(v) = self.window_to_virtual(x, y) {
            self.cursor_virtual = v;
        }
        if button == 1 {
            self.mouse_left_down = true;
            self.drag_start = Some((x, y));
            self.dragging = false;
        }
        // World map (mode 5). Clicks inside the map rectangle are travel/empty
        // cells; clicks on the still-visible left panel keep their original
        // role (the compass closes the map, party portraits select).
        if self.map_open {
            if let Some((vx, vy)) = self.window_to_virtual(x, y) {
                let (vx, vy) = (vx.round() as i32, vy.round() as i32);
                if self.map_click(vx, vy) {
                    self.drag = false;
                    self.last_mouse = (x, y);
                    return;
                }
                if button == 1 {
                    if let Some(code) = crate::hud::button_at(vx, vy, VIRTUAL_W) {
                        if code == crate::hud::CODE_MAP {
                            self.toggle_map();
                            return;
                        }
                    }
                    if let Some(cell) = self.party_cell_at(x, y) {
                        self.select_party_member(cell);
                        return;
                    }
                }
            }
            self.drag = false;
            self.last_mouse = (x, y);
            return;
        }
        // Quest log (mode 2): the book closes it; the rest is modal.
        if self.quest_log_open {
            if button == 1 {
                if let Some((vx, vy)) = self.window_to_virtual(x, y) {
                    let (vx, vy) = (vx.round() as i32, vy.round() as i32);
                    if let Some(code) = crate::hud::button_at(vx, vy, VIRTUAL_W) {
                        if code == crate::hud::CODE_QUEST {
                            self.toggle_quest_log();
                            return;
                        }
                    }
                    if let Some(cell) = self.party_cell_at(x, y) {
                        self.select_party_member(cell);
                        return;
                    }
                }
            }
            self.drag = false;
            self.last_mouse = (x, y);
            return;
        }
        // Exchange window: consume every click; no world input or click-through.
        if self.trade.is_some() {
            if button == 1 {
                if let Some((vx, vy)) = self.window_to_virtual(x, y) {
                    let (vx, vy) = (vx.round() as i32, vy.round() as i32);
                    match crate::trade_screen::hit_test(vx, vy, VIRTUAL_W) {
                        crate::trade_screen::TradeHit::Cell { row, col } => {
                            self.trade_click_cell(row, col)
                        }
                        crate::trade_screen::TradeHit::PageLeft(row) => self.trade_page(row, -1),
                        crate::trade_screen::TradeHit::PageRight(row) => self.trade_page(row, 1),
                        crate::trade_screen::TradeHit::Cancel => self.trade_cancel(),
                        crate::trade_screen::TradeHit::Confirm => self.trade_confirm(),
                        crate::trade_screen::TradeHit::None => {}
                    }
                }
            }
            self.drag = false;
            self.last_mouse = (x, y);
            return;
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
        // targeting and no click-through. Clicks drive the cursor/held-item
        // inventory model; a portrait click switches the edited member.
        if self.character_screen {
            if button == 1 {
                if let Some(cell) = self.party_cell_at(x, y) {
                    self.select_party_member(cell);
                    if let Some(owner) = self.owner_for_cell(cell) {
                        self.set_inventory_owner(owner);
                    }
                    return;
                }
                if let Some((vx, vy)) = self.window_to_virtual(x, y) {
                    let (vx, vy) = (vx.round() as i32, vy.round() as i32);
                    let equipment = self.owner_equipment(self.inventory_owner);
                    let raw = self.owner_raw_0x12(self.inventory_owner);
                    let hit = crate::character_screen::hit_test(vx, vy, VIRTUAL_W, &equipment, raw);
                    match hit {
                        crate::character_screen::ScreenHit::Main(slot) => {
                            let loc = crate::inventory::ItemLocation::Main(slot);
                            if self.held_item.is_some() {
                                if !self.try_craft_held_at(self.inventory_owner, loc) {
                                    self.place_held(loc);
                                }
                            } else {
                                self.lift_item(loc);
                            }
                        }
                        crate::character_screen::ScreenHit::Additional(slot) => {
                            let loc = crate::inventory::ItemLocation::Additional(slot);
                            if self.held_item.is_some() {
                                if !self.try_craft_held_at(self.inventory_owner, loc) {
                                    self.place_held(loc);
                                }
                            } else {
                                self.lift_item(loc);
                            }
                        }
                        crate::character_screen::ScreenHit::Special => {
                            let loc = crate::inventory::ItemLocation::Special;
                            if self.held_item.is_some() {
                                if !self.try_craft_held_at(self.inventory_owner, loc) {
                                    self.place_held(loc);
                                }
                            } else {
                                self.lift_item(loc);
                            }
                        }
                        crate::character_screen::ScreenHit::Inventory(slot) => {
                            let idx = self.inventory_scroll + slot;
                            let loc = crate::inventory::ItemLocation::Inventory(idx);
                            if self.held_item.is_some() {
                                if !self.try_craft_held_at(self.inventory_owner, loc) {
                                    self.deposit_held();
                                }
                            } else {
                                self.lift_item(loc);
                            }
                        }
                        crate::character_screen::ScreenHit::RaiseLevel => {
                            let _ = self.level_up_owner(self.inventory_owner);
                        }
                        crate::character_screen::ScreenHit::Raise(stat) => {
                            let _ = self.raise_owner_stat(self.inventory_owner, stat);
                        }
                        crate::character_screen::ScreenHit::None => {}
                    }
                }
            }
            return;
        }
        // A left-click on a party-portrait cell selects that member
        // (`FUN_00424D8C`; additive with Shift) and is consumed. It must not
        // issue a world movement command. The character screen is not opened by
        // this click (original: opened by a command) — use `C` instead.
        if button == 1 && !self.mode_graph {
            // Original HUD button strip (`0x81CF68`). The command compiler
            // returns codes 1..=7 before the world/inventory commands.
            if self.held_item.is_none() {
                if let Some((vx, vy)) = self.window_to_virtual(x, y) {
                    let (vx, vy) = (vx.round() as i32, vy.round() as i32);
                    let (ax, ay, aw, ah) = self.auto_rect();
                    if vx >= ax && vx < ax + aw && vy >= ay && vy < ay + ah {
                        self.toggle_auto_xp();
                        self.drag = false;
                        self.last_mouse = (x, y);
                        return;
                    }
                    if let Some(code) = crate::hud::button_at(vx, vy, VIRTUAL_W) {
                        if self.press_hud_button(code) {
                            self.drag = false;
                            self.last_mouse = (x, y);
                            return;
                        }
                    }
                }
            }
            if let Some(cell) = self.party_cell_at(x, y) {
                if self.held_item.is_some() {
                    self.give_held_to_member(cell);
                } else {
                    self.select_party_member(cell);
                }
                return;
            }
            // Inventory strip: lift with an empty cursor, deposit with a held
            // item. It must never be interpreted as a world move.
            if let Some(slot) = self.hovered_slot {
                let idx = self.inventory_scroll + slot;
                let loc = crate::inventory::ItemLocation::Inventory(idx);
                if self.held_item.is_some() {
                    if !self.try_craft_held_at(self.inventory_owner, loc) {
                        self.deposit_held();
                    }
                } else {
                    self.lift_item(loc);
                }
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
            } else if self.held_item.is_some() {
                // Original command 0: left-clicking the world while carrying an
                // item drops it at the source owner's cell (`FUN_004138C8`).
                self.drop_held_on_ground();
            } else {
                self.world_move(x, y);
            }
            self.drag = false;
            self.last_mouse = (x, y);
        } else if button == 3 {
            // Right button: use an inventory item, otherwise world interaction.
            if !self.mode_graph {
                if let Some(slot) = self.hovered_slot {
                    if self.held_item.is_none() {
                        self.right_click_inventory_slot(slot);
                        self.drag = false;
                        self.last_mouse = (x, y);
                        return;
                    }
                }
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
    pub(super) fn world_move(&mut self, x: f64, y: f64) {
        if self.player.dead {
            return;
        }
        // Left-click move clears `Entity+0x03` bit `0x40` on every selected
        // unit; ordinary walk-over therefore never collects. A new move order
        // also replaces any pending melee target.
        self.player_interact = false;
        self.player_attack_target = None;
        self.player_attack_ranged = false;
        for m in self.party.iter_mut() {
            m.motion.combat_target = None;
            m.motion.ranged_order = false;
        }
        self.world_move_command(x, y);
    }

    fn world_move_command(&mut self, x: f64, y: f64) {
        // `FUN_0042260C`: the first world click consumes `_DAT_0081D660 == 7`.
        self.dig_armed = false;
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
        if interact && (self.player_x.round() as i32, self.player_y.round() as i32) == (tx, ty) {
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
        // `FUN_0042260C` resets `_DAT_0081D660` from 7 to 4 on any world click.
        self.dig_armed = false;
        let Some((vx, vy)) = self.window_to_virtual(x, y) else {
            return;
        };
        let (vx, vy) = (vx.round() as i32, vy.round() as i32);
        if let Some((entity, archetype, dialogue_id, _group_id)) = self.actor_at(vx, vy) {
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
        // Prop/object under the cursor (`_DAT_0081D57C`): a ranged prop order
        // only, using the original ammo `+0x02 == 0` gate.
        let lx = vx - self.detail_pan[0].round() as i32;
        let ly = vy - self.detail_pan[1].round() as i32;
        if let Some((prop, gx, gy)) = self.world.prop_at_local(self.current_map, lx, ly) {
            let _ = (gx, gy);
            self.issue_prop_order(prop);
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
        if self.trade.is_some() {
            if button == 1 {
                self.mouse_left_down = false;
            }
            self.drag = false;
            return;
        }
        if button == 1 {
            if self.dragging {
                self.drop_held_at_cursor();
            }
            self.mouse_left_down = false;
            self.drag_start = None;
            self.dragging = false;
        }
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
        if self.mouse_left_down && self.held_item.is_some() {
            if let Some((sx, sy)) = self.drag_start {
                let dx = x - sx;
                let dy = y - sy;
                if dx * dx + dy * dy > 16.0 {
                    self.dragging = true;
                }
            }
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
        if self.trade.is_some() {
            self.hovered_slot = None;
            return;
        }
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
        if self.trade.is_some() {
            return;
        }
        if self.mode_graph {
            let factor = 1.2f64.powf(delta);
            self.graph_zoom = (self.graph_zoom * factor).clamp(0.04, 3.0);
            return;
        }
        if self.hovered_slot.is_some() {
            let l = crate::hud::layout(VIRTUAL_W);
            let max = crate::inventory::INVENTORY_SLOTS.saturating_sub(l.visible_slots);
            let step = if delta > 0.0 { 1 } else { -1 };
            self.inventory_scroll =
                (self.inventory_scroll as i32 + step).clamp(0, max as i32) as usize;
        }
    }

    pub fn key_down(&mut self, code: u32) {
        // Exchange captures the keyboard: Escape cancels/returns the offers.
        if self.trade.is_some() {
            if code == KEY_ESCAPE {
                self.trade_cancel();
                self.hovered_slot = None;
            }
            return;
        }
        // Dialogue captures the keyboard: only Escape closes it.
        if self.dialog.is_some() {
            if code == KEY_ESCAPE {
                self.dialog = None;
                self.hovered_slot = None;
            }
            return;
        }
        // Mode 5 / mode 2: only the original close keys act.
        if self.map_open && code != KEY_ESCAPE && code != KEY_M {
            return;
        }
        if self.quest_log_open && code != KEY_ESCAPE && code != KEY_Q {
            return;
        }
        match code {
            // Escape closes the character screen when open; otherwise it opens
            // the frontend/menu (the live game is preserved by the host runtime).
            KEY_ESCAPE => {
                if self.map_open {
                    self.toggle_map();
                } else if self.quest_log_open {
                    self.toggle_quest_log();
                } else if self.character_screen {
                    self.character_screen = false;
                    self.hovered_slot = None;
                } else {
                    self.request_menu = true;
                }
            }
            // M — world map toggle (`FUN_0042CBD0(4)`, VK 0x4D).
            KEY_M => {
                self.toggle_map();
            }
            // Q — quest log (`FUN_004242BC`, mode 2).
            KEY_Q => {
                self.toggle_quest_log();
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
            // SPACE — original `FUN_00421554` battle/peace stance toggle.
            KEY_SPACE | KEY_TAB => {
                if !self.mode_graph {
                    self.toggle_selected_battle_stance();
                }
            }
            // A — original `FUN_00421458` party battle-engage. Shift+A selects
            // every visible party unit instead (the original Shift+A branch).
            KEY_A => {
                if !self.mode_graph {
                    if self.shift_down {
                        self.select_all_party();
                    } else {
                        self.party_engage();
                    }
                }
            }
            // 1/2/3 — original `FUN_004216DC`: select weapon slot 2/0/1.
            KEY_1 => {
                if !self.mode_graph {
                    self.select_weapon_slot(2);
                }
            }
            KEY_2 => {
                if !self.mode_graph {
                    self.select_weapon_slot(0);
                }
            }
            KEY_3 => {
                if !self.mode_graph {
                    self.select_weapon_slot(1);
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
            // F — original `FUN_0042246C` party halt (camera fit is Home now).
            KEY_F => {
                if !self.mode_graph {
                    self.party_halt();
                }
            }
            KEY_HOME => {
                if !self.mode_graph {
                    self.fit_detail();
                    if self.camera_follow {
                        self.center_camera_on_player(true, 0.0);
                    }
                }
            }
            // `C` / `I` toggle the character screen (the original opens it via
            // `FUN_00424330`, bound to VK_I; C is the remake alias).
            KEY_C | KEY_I => {
                if !self.mode_graph {
                    self.toggle_character_screen();
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
