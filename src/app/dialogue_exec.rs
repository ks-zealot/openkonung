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
//! Recruitable-companion opcodes: `0x123` (`FUN_00431820`, the NPC joins the
//! reserved party **group 0**) and `0x14C` (`FUN_00432954`, dismissed); gold ops
//! `0x11D..0x12A` adjust `Entity+0x34` (`PlayerCharacter::money`).

use super::*;

use crate::actors::{ActorMotion, PartyMember};
use crate::script::{action_steps, evaluate_condition, ActionStep};

/// Snapshot of the runtime state a `B4`/`B5` program can read. Built once per
/// page build / option select so condition evaluation never borrows the app
/// while the dialogue state is mutated.
#[derive(Clone, Debug, Default)]
pub struct QuestContext {
    pub money: i32,
    /// Party member count including the protagonist (`Group+0x1C`).
    pub party_count: usize,
    /// `max_party` from class + charisma (`FUN_00411078`).
    pub party_capacity: usize,
    /// `_DAT_0081D580 != 0` (a settlement is present at this location).
    pub settlement_present: bool,
    pub npc_archetype: u8,
    pub npc_level: u8,
    /// The dialogue NPC is a member of the party (`Entity+0x05 == 0`).
    pub npc_in_party: bool,
    /// Runtime `B7` flag bytes.
    pub flags: Vec<u8>,
}

impl QuestContext {
    /// `B7` slot active bit (`0x80`).
    pub fn flag_active(&self, index: usize) -> bool {
        self.flags.get(index).map(|b| b & 0x80 != 0).unwrap_or(false)
    }
}

/// Evaluate a script-opcode predicate (`FUN_0043159C`) for op `op`.
///
/// Only the predicates used by the shipped dialogue/quest data are recovered
/// (see `script.rs` / `QUEST_OPCODES.md`); an unrecognized predicate returns
/// `false` so an unknown condition never spuriously enables an option.
fn eval_op(ctx: &QuestContext, op: u16) -> bool {
    match op {
        // 0x115 rng()%100 < 50. Deterministic approximation on this path (the
        // shipped recruit flow does not depend on it).
        0x115 => true,
        // 0x12E settlement flag 0x10 (not modeled).
        0x12E => false,
        // 0x12F protagonist dialogue id (not modeled).
        0x12F => false,
        0x131 => ctx.money > 29,
        0x132 => ctx.money >= 40,
        0x133 => ctx.money >= 50,
        0x134 => ctx.money >= 100,
        0x135 => ctx.money >= 500,
        0x136 => ctx.money >= 300,
        0x138 => ctx.settlement_present,
        // 0x15D FUN_00434168: max_party <= party count (party is at capacity).
        0x15D => ctx.party_capacity <= ctx.party_count,
        // 0x15E FUN_00434228: Entity+5 == 0 (member of the reserved party group).
        0x15E => ctx.npc_in_party,
        // 0x160 FUN_004342A4: Entity+0x33 (level) > 3.
        0x160 => ctx.npc_level > 3,
        // 0x161 Entity+4 & 0x38 != 0 (trained specialist / instructor).
        0x161 => ctx.npc_archetype & 0x38 != 0,
        _ => false,
    }
}

impl PlayableApp {
    /// Build the condition/action snapshot for a dialogue with `npc_entity`.
    pub(super) fn quest_context(&self, npc_entity: usize) -> QuestContext {
        let e = self.tables.as_ref().and_then(|t| t.entities.get(npc_entity));
        let (archetype, level) = e.map(|e| (e.archetype, e.stats.level)).unwrap_or((0, 0));
        let capacity = crate::character::max_party(self.player.class, self.player.charisma_base);
        QuestContext {
            money: self.player.money,
            party_count: 1 + self.party.len(),
            party_capacity: capacity as usize,
            settlement_present: self
                .settlements
                .iter()
                .any(|s| s.loc_id as u32 == self.current_map),
            npc_archetype: archetype,
            npc_level: level,
            npc_in_party: self.party.iter().any(|m| m.entity.index == npc_entity),
            flags: self.quest_flags.clone(),
        }
    }

    /// Run `B5` action program `id` (`FUN_00434BB8`).
    pub(super) fn run_action_program(&mut self, id: u16, npc_entity: usize) {
        let words = self
            .world
            .res
            .quest
            .as_ref()
            .map(|q| q.action_program(id))
            .unwrap_or_default();
        for step in action_steps(&words) {
            match step {
                ActionStep::Op(op) => self.exec_op(op, npc_entity),
                ActionStep::SetFlag(i) => {
                    if let Some(b) = self.quest_flags.get_mut(i) {
                        *b |= 0x80;
                    }
                }
                ActionStep::ClearFlag(i) => {
                    if let Some(b) = self.quest_flags.get_mut(i) {
                        *b &= !0x80;
                    }
                }
            }
        }
    }

    /// Execute one script opcode (`FUN_0043159C`) for a dialogue action program.
    fn exec_op(&mut self, op: u16, npc_entity: usize) {
        match op {
            // 0x123 FUN_00431820: recruit the dialogue NPC into the party.
            0x123 => self.recruit_companion(npc_entity),
            // 0x14C FUN_00432954: dismiss the party member.
            0x14C => self.dismiss_companion(npc_entity),
            0x11D => self.player.money -= 5,
            0x11E => self.player.money -= 10,
            0x11F => self.player.money -= 20,
            0x120 => self.player.money -= 50,
            0x121 => self.player.money -= 100,
            0x122 => self.player.money -= 250,
            0x125 => self.player.money += 5,
            0x126 => self.player.money += 10,
            0x127 => self.player.money += 20,
            0x128 => self.player.money += 50,
            0x129 => self.player.money += 100,
            0x12A => self.player.money += 250,
            0x12B => self.player.available_xp = self.player.available_xp.saturating_add(10),
            0x12C => self.player.available_xp = self.player.available_xp.saturating_add(20),
            0x12D => self.player.available_xp = self.player.available_xp.saturating_add(50),
            _ => {}
        }
    }

    /// Add the dialogue NPC as a party companion (original recruit `0x123`).
    ///
    /// The canonical `Entity` is taken verbatim from the parsed `Game.N` tables;
    /// a fresh [`ActorMotion`] is seeded at the NPC's current cell. The NPC is
    /// removed from its location group (`FUN_00431820` moves the entity into the
    /// reserved party group 0 and compacts the old group), so it no longer
    /// renders as a world NPC; it is rendered as a follower instead. The new
    /// member is auto-selected into the party selection set.
    pub(super) fn recruit_companion(&mut self, npc_entity: usize) {
        if self.party.iter().any(|m| m.entity.index == npc_entity) {
            return;
        }
        let Some(entity) = self
            .tables
            .as_ref()
            .and_then(|t| t.entities.get(npc_entity))
            .cloned()
        else {
            return;
        };
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
            .walk_cycle_frames(entity.archetype, entity.visual_index, facing as usize, battle)
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
        self.party.push(PartyMember { entity, motion });

        // Remove the NPC from the world population (it left its location group).
        let loc = self.current_map;
        if let Some(pop) = self.population_cache.get_mut(&loc) {
            pop.actors.retain(|a| a.source_entity_id() != npc_entity);
            pop.motion.remove(&npc_entity);
        }

        // Auto-select the new member (`FUN_00424D8C` adds it to the selection).
        let cell_index = self.party.len();
        if cell_index < crate::hud::PARTY_CELLS {
            self.party_selected[cell_index] = true;
        }
    }

    /// Remove a companion from the party (original dismiss `0x14C`). Selection
    /// slots at and after the removed cell shift down. The location population is
    /// dropped so the NPC is rebuilt back into the world on the next tick.
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

    /// Build and open the dialogue for `dialogue_id` on `npc_entity`
    /// (`FUN_00435290` + `FUN_00434C74` + `FUN_0042CBD0(0)`).
    pub(super) fn open_dialog(&mut self, npc_entity: usize, dialogue_id: u8) {
        let fw = VIRTUAL_W;
        let ctx = self.quest_context(npc_entity);
        let started = {
            let Self { world, .. } = self;
            world.res.quest.as_ref().and_then(|q| {
                crate::dialog::DialogState::start(
                    q,
                    &world.res.fonts,
                    npc_entity,
                    dialogue_id,
                    fw,
                    &|id: u16| {
                        let words = q.condition_program(id);
                        evaluate_condition(&words, |i| ctx.flag_active(i), |op| eval_op(&ctx, op))
                    },
                )
            })
        };
        if let Some(state) = started {
            self.dialog = Some(state);
            // NPC voice line (`FUN_00435290` picks 0x0C..0x0F for a humanoid).
            self.sounds.push(0x0C + (npc_entity as u16 & 3));
            self.run_on_open_actions(npc_entity);
        }
    }

    /// Run the `B5` actions queued by the current page's statement entries.
    fn run_on_open_actions(&mut self, npc_entity: usize) {
        let actions = self
            .dialog
            .as_ref()
            .map(|d| d.on_open_actions.clone())
            .unwrap_or_default();
        for id in actions {
            self.run_action_program(id, npc_entity);
        }
    }

    /// Select the option under the cursor: run its `B5` action, then advance.
    pub(super) fn dialog_click_option(&mut self, index: usize) {
        let Some(npc_entity) = self.dialog.as_ref().map(|d| d.npc_entity) else {
            return;
        };
        // Run the option's action program before following its link.
        if let Some(action) = self.dialog.as_ref().and_then(|d| d.options.get(index).map(|o| o.action))
        {
            if action >= 0 {
                self.run_action_program(action as u16, npc_entity);
            }
        }
        // Resolve the next page with a fresh context (the action may have set
        // flags / changed the party).
        let ctx = self.quest_context(npc_entity);
        let stay = {
            let Self { dialog, world, .. } = self;
            let (Some(quest), Some(state)) = (world.res.quest.as_ref(), dialog.as_mut()) else {
                *dialog = None;
                return;
            };
            state.select(
                quest,
                &world.res.fonts,
                index,
                VIRTUAL_W,
                &|id: u16| {
                    let words = quest.condition_program(id);
                    evaluate_condition(&words, |i| ctx.flag_active(i), |op| eval_op(&ctx, op))
                },
            )
        };
        if stay {
            self.run_on_open_actions(npc_entity);
        } else {
            self.dialog = None;
        }
    }
}
