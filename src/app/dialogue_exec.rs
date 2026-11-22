//! `dialogue_exec` subsystem of [`crate::app`] (split from `app.rs`).
//!
//! Runs the original `QUEST_R.RES` condition (`B4`) and action (`B5`) programs
//! while a dialogue is open, and drives the NPC dialogue interface:
//!
//! - **Open** (`FUN_0042260C` right button -> `FUN_00435290`): the NPC's
//!   `Entity+0xD0` dialogue id indexes `B2` for the first node.
//! - **Page build** (`FUN_00434C74`): each entry's `B4` program is evaluated with
//!   `FUN_00434FE8`; conditional (f7) entries whose condition holds become the
//!   NPC statement lines (a passing statement's `B5` action runs immediately),
//!   and unconditional (f6) entries become the selectable options (shown when
//!   their condition is absent or holds). A branch page (`+0x0A == -2`) selects
//!   the first case whose condition holds and follows its `+0x04`.
//! - **Select** (`FUN_0042260C` case `0x65`): the option's `B5` action program is
//!   run (`FUN_00434BB8`) before following the option's `+0x04`.
//!
//! Every opcode is executed by [`QuestContext`] (`dialogue_ops.rs`), including
//! the `Both` kind during condition evaluation. The context is an owned snapshot
//! applied back once per page/selection, so the quest borrow and the mutable
//! game state never overlap.

use super::*;

use super::dialogue_ops::QuestContext;
use crate::actors::ActorMotion;
use crate::script::evaluate_condition;

/// `FUN_00435290` window template / voice id.
///
/// Humanoids (`archetype & 0x40 == 0`) draw `rand()%4 + 0x0C`; when that equals
/// the previous template (`_DAT_0081D68C`) they re-draw `rand()%4 + 0x0D` and
/// map `0x10` back to `0x0C`. Object NPCs draw nothing: class `Q`/`R`/`S` map
/// to `0x11`/`0x12`/`0x13`, anything else to `0x10`. The same id is the NPC
/// voice clip (`FUN_0042A13C`) and selects the dialogue window art.
pub(super) fn dialog_template(
    archetype: u8,
    previous: u8,
    rng: &mut crate::combat::OriginalRng,
) -> u8 {
    if archetype & 0x40 == 0 {
        let mut template = (rng.next_u32() % 4) as u8 + 0x0C;
        if template == previous {
            template = (rng.next_u32() % 4) as u8 + 0x0D;
            if template == 0x10 {
                template = 0x0C;
            }
        }
        template
    } else {
        match archetype {
            b'Q' => 0x11,
            b'R' => 0x12,
            b'S' => 0x13,
            _ => 0x10,
        }
    }
}

impl PlayableApp {
    /// Build and open the dialogue for `dialogue_id` on `npc_entity`
    /// (`FUN_00435290` + `FUN_00434C74` + `FUN_0042CBD0(0)`).
    pub(super) fn open_dialog(&mut self, npc_entity: usize, dialogue_id: u8) {
        let fw = VIRTUAL_W;
        // `FUN_00435290`: pick the window template / voice id before resolving
        // the page (the humanoid branch consumes one or two `rand()%4` draws).
        let archetype = self
            .party
            .iter()
            .find(|m| m.entity.index == npc_entity)
            .map(|m| m.entity.archetype)
            .or_else(|| {
                self.tables
                    .as_ref()
                    .and_then(|t| t.entities.get(npc_entity))
                    .map(|e| e.archetype)
            })
            .unwrap_or(0);
        let template = dialog_template(archetype, self.dialog_template, &mut self.sound_rng);
        self.dialog_template = template;
        let mut ctx = QuestContext::from_app(self, npc_entity);
        let started = {
            let world = &self.world;
            world.res.quest.as_ref().and_then(|q| {
                crate::dialog::DialogState::start(
                    q,
                    &world.res.fonts,
                    npc_entity,
                    dialogue_id,
                    fw,
                    &mut |id: u16| {
                        let words = q.condition_program(id);
                        let flags = ctx.quest_flags.clone();
                        evaluate_condition(
                            &words,
                            |i| flags.get(i).map(|w| w & 0x80 != 0).unwrap_or(false),
                            |op| ctx.exec_op(op),
                        )
                    },
                )
            })
        };
        if let Some(state) = started {
            self.dialog = Some(state);
            // NPC voice line: `FUN_00435290` plays the template id that was
            // chosen above (`0x0C..0x13`), once the page resolved.
            self.queue_sound(template as u16);
            let actions = self
                .dialog
                .as_ref()
                .map(|d| d.on_open_actions.clone())
                .unwrap_or_default();
            if let Some(q) = self.world.res.quest.as_ref() {
                for id in actions {
                    ctx.run_action_program(q, id);
                }
            }
            let trade = ctx.trade_request.is_some();
            ctx.apply_to_app(self);
            if trade {
                self.dialog = None;
            }
        } else {
            ctx.apply_to_app(self);
        }
    }

    /// Select the option under the cursor: run its `B5` action, then advance.
    pub(super) fn dialog_click_option(&mut self, index: usize) {
        let Some(npc_entity) = self.dialog.as_ref().map(|d| d.npc_entity) else {
            return;
        };
        let mut ctx = QuestContext::from_app(self, npc_entity);
        let action = self
            .dialog
            .as_ref()
            .and_then(|d| d.options.get(index).map(|o| o.action));
        if let Some(action) = action {
            if action >= 0 {
                if let Some(q) = self.world.res.quest.as_ref() {
                    ctx.run_action_program(q, action as u16);
                }
            }
        }

        let stay = {
            let Self { dialog, world, .. } = self;
            let (Some(quest), Some(state)) = (world.res.quest.as_ref(), dialog.as_mut()) else {
                *dialog = None;
                return;
            };
            state.select(quest, &world.res.fonts, index, VIRTUAL_W, &mut |id: u16| {
                let words = quest.condition_program(id);
                let flags = ctx.quest_flags.clone();
                evaluate_condition(
                    &words,
                    |i| flags.get(i).map(|w| w & 0x80 != 0).unwrap_or(false),
                    |op| ctx.exec_op(op),
                )
            })
        };
        if stay {
            let actions = self
                .dialog
                .as_ref()
                .map(|d| d.on_open_actions.clone())
                .unwrap_or_default();
            if let Some(q) = self.world.res.quest.as_ref() {
                for id in actions {
                    ctx.run_action_program(q, id);
                }
            }
        }
        let trade = ctx.trade_request.is_some();
        ctx.apply_to_app(self);
        if trade || !stay {
            self.dialog = None;
        }
    }

    /// Test fixture: add the dialogue NPC as a party companion (`0x123` shape).
    ///
    /// The canonical `Entity` is taken verbatim from the parsed `Game.N` tables;
    /// a fresh [`ActorMotion`] is seeded at the NPC's current cell. The NPC is
    /// removed from its location population (the original moves it into the
    /// reserved party group 0). Like `FUN_00431820`, the **selection array is
    /// not touched** - callers that need the member selected must do so
    /// explicitly (Shift+cell / `select_party_member`).
    #[allow(dead_code)]
    pub(super) fn recruit_companion(&mut self, npc_entity: usize) {
        if self.party.iter().any(|m| m.entity.index == npc_entity) {
            return;
        }
        let Some(mut entity) = self
            .tables
            .as_ref()
            .and_then(|t| t.entities.get(npc_entity))
            .cloned()
        else {
            return;
        };
        entity.group_id = 0;
        let cell = self
            .population_cache
            .get(&self.current_map)
            .and_then(|p| p.motion.get(&npc_entity))
            .map(|m| m.cell)
            .unwrap_or((entity.x as i32, entity.y as i32));
        let battle = crate::anim::battle_from_flags(entity.flags_b);
        let facing = entity.facing & 7;
        let walk_frames = self
            .world
            .res
            .walk_cycle_frames(
                entity.archetype,
                entity.visual_index,
                facing as usize,
                battle,
            )
            .unwrap_or(0)
            .min(u8::MAX as usize) as u8;
        let idle_frames = self
            .world
            .res
            .action_cycle_frames(
                entity.archetype,
                entity.visual_index,
                facing as usize,
                crate::anim::idle_action(battle),
            )
            .unwrap_or(0)
            .min(u8::MAX as usize) as u8;
        let mut motion = ActorMotion::default();
        motion.cell = cell;
        motion.render_cell = cell;
        motion.facing = facing;
        motion.walk_frames = walk_frames;
        motion.idle_frames = idle_frames;
        motion.action = crate::anim::idle_action(battle) as u8;
        self.party
            .push(crate::actors::PartyMember { entity, motion });

        // Remove the NPC from the world population (it left its location group).
        let loc = self.current_map;
        if let Some(pop) = self.population_cache.get_mut(&loc) {
            pop.actors.retain(|a| a.source_entity_id() != npc_entity);
            pop.motion.remove(&npc_entity);
        }
    }

    /// Remove a companion from the party (original dismiss `0x14C`). Selection
    /// slots at and after the removed cell shift down. The location population is
    /// dropped so the NPC is rebuilt back into the world on the next tick.
    #[allow(dead_code)]
    pub(super) fn dismiss_companion(&mut self, npc_entity: usize) {
        let Some(index) = self.party.iter().position(|m| m.entity.index == npc_entity) else {
            return;
        };
        self.party.remove(index);
        let cell = index + 1;
        for j in cell..crate::hud::PARTY_CELLS - 1 {
            self.party_selected[j] = self.party_selected[j + 1];
        }
        self.party_selected[crate::hud::PARTY_CELLS - 1] = false;
        let loc = self.current_map;
        self.population_cache.remove(&loc);
    }
}

#[cfg(test)]
mod tests {
    use super::dialog_template;
    use crate::combat::OriginalRng;

    /// Humanoid templates stay in `0x0C..=0x0F` (window art + voice clip) and
    /// the draw consumes the RNG stream.
    #[test]
    fn humanoid_template_range_and_draw() {
        let mut rng = OriginalRng::new(0x1234_5678);
        let mut prev = 0u8;
        for _ in 0..200 {
            let t = dialog_template(0x05, prev, &mut rng);
            assert!((0x0C..=0x0F).contains(&t), "humanoid template {t:#x}");
            prev = t;
        }
    }

    /// Object NPCs never draw: class `Q`/`R`/`S` map to `0x11`/`0x12`/`0x13`,
    /// anything else to `0x10`.
    #[test]
    fn object_template_is_fixed_and_consumes_no_rng() {
        let mut rng = OriginalRng::new(7);
        assert_eq!(dialog_template(b'Q', 0, &mut rng), 0x11);
        assert_eq!(dialog_template(b'R', 0, &mut rng), 0x12);
        assert_eq!(dialog_template(b'S', 0, &mut rng), 0x13);
        assert_eq!(dialog_template(0x41, 0, &mut rng), 0x10);
        let mut a = OriginalRng::new(99);
        let mut b = OriginalRng::new(99);
        let _ = dialog_template(b'Q', 0, &mut a);
        assert_eq!(a.next_u32(), b.next_u32(), "object branch draws nothing");
    }
}
