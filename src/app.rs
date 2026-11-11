//! OpenKonung live-game state machine: [`PlayableApp`] and its subsystem modules.
//!
//! [`PlayableApp`] owns every live-game global the original keeps in static
//! memory: the decoded `RES` pack, the transition graph, the parsed `Game.N`
//! `Group[200]`/`Entity[2000]`/`Settlement[20]` tables, the runtime population
//! and motion cache, the party, the runtime `B7` quest flags, the interface mode
//! flags and the 800x600 RGBA framebuffer.
//!
//! The game advances on the original **60 ms** frame budget (`0x42B32F`
//! writes `0x3C` to `0x81D5C8`), so [`PlayableApp::step`] converts the shell's
//! wall-clock `dt` into whole original frames and drives the subsystem passes
//! at that rate instead of per browser repaint. Subsystems live in sibling
//! `app/*.rs` files; each file documents its own slice of the original state
//! machine.

use std::collections::{HashMap, HashSet};

use crate::actors::{
    build_population, find_path, sort_actors, update_population, ActorMotion, PopulationBuild,
    LOGIC_TICK_HZ,
};
use crate::character::PlayerCharacter;
use crate::coords::{
    entity_to_local_px, entity_to_local_px_f, local_px_to_entity, step_by_direction,
};
use crate::edges::{Edge, Pair};
use crate::population::{GameTables, Settlement};
use crate::render::{World, WorldSprite};
use crate::res::{blit, fill_rect, Img};
use crate::world_objects::{ActiveWorldObject, WorldObjectSource};

mod buttons;
mod camera;
mod class11;
mod combat;
mod cursor;
mod dialogue_exec;
mod dialogue_ops;
mod hover;
mod input;
mod inventory;
mod lighting;
mod map;
mod party;
mod player;
mod population;
mod quest_log;
mod save;
mod settlement_defense;
mod settlement_pass;
mod settlement_targets;
mod settlement_trade;
mod settlement_training;
mod sound;
mod text;
pub(crate) mod tooltip;
mod trade;
mod video_triggers;
mod world_draw;
mod world_items;

pub use crate::constants::render::VIRTUAL_H;
pub use crate::constants::render::VIRTUAL_W;

pub use crate::constants::keys::KEY_1;
pub use crate::constants::keys::KEY_2;
pub use crate::constants::keys::KEY_3;
pub use crate::constants::keys::KEY_A;
pub use crate::constants::keys::KEY_BACKSPACE;
pub use crate::constants::keys::KEY_C;
pub use crate::constants::keys::KEY_D;
pub use crate::constants::keys::KEY_ESCAPE;
pub use crate::constants::keys::KEY_F;
pub use crate::constants::keys::KEY_H;
pub use crate::constants::keys::KEY_HOME;
pub use crate::constants::keys::KEY_I;
pub use crate::constants::keys::KEY_L;
pub use crate::constants::keys::KEY_M;
pub use crate::constants::keys::KEY_P;
pub use crate::constants::keys::KEY_Q;
pub use crate::constants::keys::KEY_R;
pub use crate::constants::keys::KEY_SPACE;
pub use crate::constants::keys::KEY_T;
pub use crate::constants::keys::KEY_TAB;
pub use crate::constants::keys::KEY_V;

/// One diagnostic/HUD text run queued for the browser shell.
///
/// The shell owns the actual glyph rasteriser (`www/app.js`, 14 px Consolas
/// scaled by `scale`), so the wasm side only describes what to draw. The
/// `text_count`/`text_x`/`text_y`/`text_rgb`/`text_scale`/`text_ptr`/
/// `text_len` ABI reads this vector after [`PlayableApp::step`].
pub struct TextItem {
    /// Virtual-pixel x of the left edge (0..=800).
    pub x: i32,
    /// Virtual-pixel y of the baseline box top (0..=600).
    pub y: i32,
    /// Packed `0xRRGGBB` colour (alpha is always opaque).
    pub rgb: u32,
    /// Multiplier applied to the shell's 14 px font.
    pub scale: f32,
    /// UTF-8 payload (CP866 names already transcoded by `crate::names`).
    pub s: String,
}

/// Approximate rendered width of a monospace text run in virtual pixels. The
/// browser shell renders `14 * scale` px Consolas (~0.6em advance); 0.62 is a
/// slight over-estimate so wrapping errs on the safe side.
fn text_px(s: &str, scale: f32) -> i32 {
    (s.chars().count() as f32 * 14.0 * scale * 0.62).ceil() as i32
}

/// Greedy word wrap to at most `max_px` virtual pixels per line, hard-splitting
/// any single word that cannot fit on its own line.
fn wrap_to_px(s: &str, scale: f32, max_px: i32) -> Vec<String> {
    let max_px = max_px.max(48);
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in s.split(' ') {
        if cur.is_empty() {
            if text_px(word, scale) > max_px {
                let mut chunk = String::new();
                for ch in word.chars() {
                    if text_px(&format!("{}{}", chunk, ch), scale) > max_px && !chunk.is_empty() {
                        lines.push(std::mem::take(&mut chunk));
                    }
                    chunk.push(ch);
                }
                cur = chunk;
            } else {
                cur.push_str(word);
            }
            continue;
        }
        let trial = format!("{} {}", cur, word);
        if text_px(&trial, scale) <= max_px {
            cur = trial;
        } else {
            lines.push(std::mem::take(&mut cur));
            cur.push_str(word);
        }
    }
    lines.push(cur);
    lines
}

/// One live game session: the replacement for the original's static globals.
///
/// Build it with [`PlayableApp::new`], feed the browser's input through
/// `pointer_down`/`pointer_move`/`pointer_up`/`wheel`/`key_down`, then call
/// [`Self::step`] once per animation frame. After each step the wasm ABI reads
/// [`Self::frame`] (800x600 RGBA), [`Self::texts`] and [`Self::sounds`].
///
/// `pub` fields are consumed by `src/abi/*` or the headless diagnostic
/// tools; the remaining fields are subsystem-internal state.
pub struct PlayableApp {
    /// Decoded resource pack: the four original RES images with their
    /// decoders, every `Konung.N` location (collision, terrain, gates) and the
    /// per-map `LIGHTS.N` masks; the attached `Game.N` save image lives inside
    /// `world.res`.
    pub world: World,
    /// Every directed transition record of the loaded profile (`edges.rs`).
    pub edges: Vec<Edge>,
    /// Bidirectional pairs: `ab`/`ba` are edge indices of the two directions.
    pub pairs: Vec<Pair>,
    /// Indices of the edges that have no reverse edge (one-way doors).
    pub one_way: Vec<usize>,
    /// Indices of the edges whose `from == to` (same-map transitions).
    pub self_edges: Vec<usize>,
    /// Normalised graph-view node coordinates (`0..=1` per axis) keyed by
    /// location id, as authored in the `Game.N` layout block.
    pub layout: HashMap<u32, (f64, f64)>,
    /// Which shipped start/variant is loaded (`Game.0`/`Game.1`/`Game.2`).
    pub profile: u32,
    location_names: Vec<String>,

    /// Current location: the 1-based `Konung.N` index (`0x81D5xx` in the
    /// original); every population, settlement and transition query is
    /// filtered on it.
    pub current_map: u32,
    /// Node id selected in the world map.
    pub selected: Option<u32>,

    /// One authoritative runtime player: hero identity + class + the
    /// character-creation values copied from the final [`crate::character::CharacterBuild`].
    pub player: PlayerCharacter,
    /// Player **committed** runtime/Entity cell (`Entity+0x08/+0x0A`). Advanced
    /// at the START of each walk segment (`FUN_0041435C`), so while a walk is in
    /// flight this is already the cell the hero is stepping INTO. Commands,
    /// teleport triggers and the camera key off this cell.
    pub player_x: f64,
    /// Committed cell y (`Entity+0x0A`), advanced with [`Self::player_x`].
    pub player_y: f64,
    /// Draw-only cell: the pre-step (source) cell of the segment currently
    /// animated. This is the original `Entity+0xE8/+0xF0`, computed by
    /// `FUN_0041435C` from `+0x08/+0x0A` *before* the step is committed, so the
    /// sprite is drawn one step behind the logical cell (the per-frame anchor
    /// sweeps it onto `player_x/player_y` over the cycle).
    pub player_render_x: f64,
    /// Draw-only cell y, paired with [`Self::player_render_x`].
    pub player_render_y: f64,
    /// Facing direction code `0..=7` (`Entity+0x01`, `coords::DIR_*`).
    pub player_dir: i32,
    target: Option<(f64, f64)>,
    /// Remaining movement-direction codes (0..=7) to `target`. Each completed
    /// walk-animation cycle consumes one code and steps one cell
    /// (`FUN_0043DA1F` queue + `FUN_0041435C`), like the original.
    player_path: Vec<u8>,
    /// Seconds accumulated toward the next player cell step.
    player_step_t: f64,
    /// True while a walk segment is animating. Kept separate from `player_path`
    /// because the final segment's direction is consumed at segment START, so
    /// the queue is already empty while the sprite is still walking.
    player_active: bool,
    anim_time: f64,
    teleport_cooldown: f64,

    // Phase C1 population (Game.N Group/Entity tables). Pending `(0,0)`
    // entities are deferred to C2; no runtime placement/AI is implemented.
    tables: Option<GameTables>,
    /// Parsed `Game.N` settlements (citizen home/role waypoints, `+0x3D0`).
    pub(crate) settlements: Vec<Settlement>,
    population_cache: HashMap<u32, PopulationBuild>,

    /// Authored `Game.N +0x21900` WorldObject source records (500 Ã— 0x40).
    world_object_sources: Vec<WorldObjectSource>,
    /// Records active on the current map (runtime marker pool `0x785BB4`).
    active_world_objects: Vec<ActiveWorldObject>,
    /// Monotonic original-frame counter (60 ms). Used by the herb respawn
    /// timestamps; unlike `anim_time` it is never reset by animation changes.
    world_frame: u64,
    world_frame_accum: f64,
    /// `Entity+0x03` bit `0x40`: set by a right-click interaction order,
    /// cleared by a left-click move or on arrival. Pickup only happens when
    /// this is set (ordinary walk-over does not collect).
    player_interact: bool,
    /// Timed feedback text for the original pickup messages.
    pickup_message: Option<String>,
    pickup_message_t: f64,
    /// Original leader final/game-over flag (`Entity+0x04 & 0x80`); the movie
    /// trigger is latched once so it is not requested every frame.
    pub(super) game_over: bool,

    // Shadow spans, exposed by the `debug_shadow_span*` ABI.
    shadow_spans: usize,
    /// Last shadow spans (bounded), for the `debug_shadow_span*` ABI.
    shadow_debug: Vec<crate::render::spans::Span>,

    detail_pan: [f64; 2],
    camera_follow: bool,
    camera_follow_strength: f64,

    // presentation
    /// 800x600 RGBA framebuffer, row-major, read by `frame_ptr`/`frame_w`/`frame_h`.
    pub frame: Vec<u8>,
    /// Text runs to draw over the frame; cleared and rebuilt every step.
    pub texts: Vec<TextItem>,
    /// Sound requests queued this frame (drained by the browser shell). Mirrors
    /// the original `FUN_0042A13C(id, volume, pan, loop)` playback calls,
    /// including the positional volume/pan and secondary-buffer rate.
    pub sounds: Vec<crate::sound::SoundEvent>,
    /// `Entity` index the protagonist was loaded from. The shipped party always
    /// uses `0`; a legacy `KONUNG.SAn` header stores it explicitly and the
    /// exporter writes it back (`_DAT_0081D56C - 0x78A250) / 0x100`).
    pub leader_entity_index: usize,
    /// Visible inventory slot under the mouse (HUD hover; read-only).
    hovered_slot: Option<usize>,
    /// Character screen (`INTR[2]`) open. Escape closes it.
    pub character_screen: bool,
    /// World-map / travel screen (original interface mode 5).
    pub map_open: bool,
    /// Quest-log window (original interface mode 2).
    pub quest_log_open: bool,
    /// Short-lived pressed HUD icon (`0x81CF68[i] |= 0x80` flash).
    pub(super) hud_press: Option<(u8, f64)>,
    /// Cached revealed-map overlay (hash of the discovered mask + image).
    pub(super) map_overlay: Option<(u64, crate::res::Img)>,
    /// Active NPC dialogue (original `FUN_0042CBD0` mode 1). Escape closes it.
    pub dialog: Option<crate::dialog::DialogState>,
    /// Active exchange (interface mode 7), opened by quest opcode `0x113`.
    pub trade: Option<crate::trade::TradeState>,
    /// Timed trade rejection message (`FUN_0042E9CC`).
    pub trade_message: Option<String>,
    /// Seconds left before [`Self::trade_message`] is cleared.
    pub trade_message_t: f64,
    /// Settlement merchant stock/restock state (`Game.N +0xA769A` tails).
    pub settlement_trade: Vec<crate::population::SettlementTrade>,
    /// Host canvas width in device pixels, set by `on_resize`.
    pub host_w: i32,
    /// Host canvas height in device pixels, set by `on_resize`.
    pub host_h: i32,
    drag: bool,
    last_mouse: (f64, f64),
    /// Set by the ABI when the session should end (never by gameplay itself).
    pub quit: bool,
    /// Set by Escape: the host runtime should open the frontend/menu while
    /// preserving this live game state (do **not** quit or rebuild).
    pub request_menu: bool,
    /// 10-slot party selection set (`FUN_00411708`, array `0x81A814`). Slot 0
    /// is the protagonist/leader and is selected at game start. Only selected
    /// members receive move/attack commands.
    pub party_selected: [bool; crate::hud::PARTY_CELLS],
    /// Recruited companions (original reserved **group 0** members added by the
    /// dialogue recruit opcode `0x123`). Slot 0 of the party is the protagonist
    /// (`self.player`); companion `i` occupies HUD cell `i + 1`.
    pub party: Vec<crate::actors::PartyMember>,
    /// Runtime `B7` quest-flag bytes (300 slots, first byte; bit `0x80` =
    /// active). Initialized from the loaded `QUEST_R.RES` at game start and
    /// mutated by dialogue action programs (`FUN_00434BB8`) / read by condition
    /// programs (`FUN_00434FE8`).
    pub quest_flags: Vec<u32>,
    /// 56-byte discovered world-map location mask (`0x81CF80` in the original).
    pub discovered_locations: [u8; 56],
    /// Additive-selection modifier (`_DAT_0081D624`, Shift): when held, a
    /// party-portrait click adds the member instead of replacing the set.
    pub shift_down: bool,
    /// Force-talk modifier (`_DAT_0081D5E4`, Ctrl): over a party member the
    /// original cursor shows the talk frame and a left-click opens dialogue.
    pub ctrl_down: bool,
    /// `_DAT_0081D628` — the class-11 torch (`Факел`) highlights ground-object
    /// markers in `FUN_0042532C` until the next map load.
    pub torch_reveal: bool,
    /// `_DAT_0081D660 == 7` — the class-11 shovel (`Лопата`) arms the dig
    /// cursor; the next world click clears it (`FUN_0042260C`).
    pub dig_armed: bool,
    /// Character whose inventory/equipment the character screen and HUD strip
    /// currently edit (original `_81D560`: protagonist or a party member).
    pub inventory_owner: crate::inventory::InventoryOwner,
    /// Item lifted onto the cursor (`_81D670`).
    pub held_item: Option<crate::inventory::HeldItem>,
    /// Visible inventory strip scroll offset (`_81D708`).
    pub inventory_scroll: usize,
    /// Fractional logic-frame accumulator for temporary potion effects.
    temp_effect_accum: f64,
    /// Left mouse button currently down (for drag detection).
    mouse_left_down: bool,
    /// Press position of the current lift, used to start a drag.
    drag_start: Option<(f64, f64)>,
    /// A lifted item is being dragged (moved past the click threshold).
    dragging: bool,
    /// Seconds accumulated toward the next companion follow-logic tick.
    party_step_t: f64,
    /// Virtual-space pointer position (`_DAT_0081D5C0/_D5C4`). The game-drawn
    /// cursor (see `app::cursor`, `res::cursor`) is blitted here. `(-1,-1)` means
    /// off-screen / not shown.
    cursor_virtual: (f64, f64),

    // --- melee combat (`combat.rs`) ---
    /// Fixed-rate accumulator for the combat/DOT tick.
    pub(super) combat_step_t: f64,
    /// Original LCG stream used by hit/dodge/loot rolls.
    pub(super) combat_rng: crate::combat::OriginalRng,
    /// Cosmetic-only LCG for ambient/action chance gates. Kept separate from
    /// `combat_rng` so adding sound rolls cannot change combat outcomes.
    pub(super) sound_rng: crate::combat::OriginalRng,
    /// Logic-frame counter; the DOT pass runs on `& 0xF == 0` (every 16 frames).
    pub(super) combat_frame: u64,
    /// Protagonist melee target (`None` = not fighting).
    pub(super) player_attack_target: Option<crate::combat::CombatTarget>,
    /// Protagonist persistent engage order (`Entity+0x02 & 0x01`, `FUN_00421458`
    /// / right-click). Keeps `FUN_0041E2D4` re-acquiring the next hostile after
    /// a kill until no enemy group is left around the leader.
    pub(super) player_engaged: bool,
    /// Current frame inside the protagonist attack cycle.
    pub(super) player_attack_frame: u8,
    /// Protagonist attack-cycle length for the current stance.
    pub(super) player_attack_frames: u8,
    /// Protagonist ranged order (bow + ammo) and action-row-4 cycle.
    pub(super) player_attack_ranged: bool,
    pub(super) player_ranged_frame: u8,
    pub(super) player_ranged_frames: u8,
    pub(super) player_ranged_fired: bool,
    /// Live ranged projectiles (original `0x80B0D0` pool, session-local).
    pub(super) projectiles: Vec<crate::combat::projectile::Projectile>,
    /// Protagonist death-cycle length/frame (final frame is held).
    pub(super) player_death_frames: u8,
    pub(super) player_death_frame: u8,
    /// Timed combat feedback text (diagnostic HUD line).
    pub(super) combat_message: Option<String>,
    pub(super) combat_message_t: f64,
    /// Diagnostic-only target retained after the order completes (smoke test).
    pub(super) combat_test_target: Option<usize>,
    pub(super) combat_test_prop: Option<usize>,
    pub(super) combat_attacks: u64,
    pub(super) combat_hits: u64,
}

impl PlayableApp {
    /// Build one live game session from the already-loaded resources.
    ///
    /// This is the remake's `FUN_00436214` state-8 ("new game") entry: it
    /// seeds the runtime `B7` quest flags from the loaded `QUEST_R.RES`, parses
    /// the 500 `Game.N +0x21900` WorldObject source records and the settlement
    /// merchant stocks, selects the protagonist in party cell 0
    /// (`FUN_004246C0` sets `selection[0] = leader`) and primes the portal
    /// bindings, settlement target props, active ground markers and camera.
    ///
    /// Nothing is randomised here and no authored value is regenerated: every
    /// field not supplied by the caller starts at a documented zero/default and
    /// is filled by the normal per-frame passes.
    ///
    /// # Arguments
    ///
    /// * `world` - decoded pack with `Game.N` already attached through
    ///   [`crate::res::Res::set_save_game`].
    /// * `edges`/`pairs`/`one_way`/`self_edges` - the transition graph
    ///   (`crate::edges`) built by the loader from every `Game.N` record.
    /// * `layout` - normalised graph-view node positions per location id.
    /// * `profile` - which shipped start (`Game.0`/`1`/`2`) is in `world`.
    /// * `player` - the finished character-creation result; copied once, never
    ///   regenerated after this point.
    /// * `start_map`/`start_x`/`start_y`/`start_dir` - initial runtime cell and
    ///   facing (`Entity+0x08/+0x0A/+0x01`), taken from the arrival record.
    /// * `tables` - parsed `Group[200]`/`Entity[2000]` population, or `None`
    ///   when no save game was loaded.
    /// * `settlements` - the 20 parsed `Game.N` settlement records.
    ///
    /// # Original
    ///
    /// `FUN_00436214` case 8 (map load), `FUN_0043ADF0` marker rebuild.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        world: World,
        edges: Vec<Edge>,
        pairs: Vec<Pair>,
        one_way: Vec<usize>,
        self_edges: Vec<usize>,
        layout: HashMap<u32, (f64, f64)>,
        profile: u32,
        location_names: Vec<String>,
        player: PlayerCharacter,
        start_map: u32,
        start_x: f64,
        start_y: f64,
        start_dir: i32,
        tables: Option<GameTables>,
        settlements: Vec<Settlement>,
    ) -> PlayableApp {
        // Runtime `B7` flag array (300 slots), initialized from the loaded
        // dialogue database exactly like the original loader.
        let quest_flags = world
            .res
            .quest
            .as_ref()
            .map(|q| q.initial_flags())
            .unwrap_or_default();
        let world_object_sources = crate::world_objects::parse_sources(&world.res.save_game);
        let settlement_trade =
            crate::population::parse_settlement_trade(&world.res.save_game).unwrap_or_default();
        let mut app = PlayableApp {
            world,
            edges,
            pairs,
            one_way,
            self_edges,
            layout,
            profile,
            location_names,
            current_map: start_map,
            selected: Some(start_map),
            player,
            player_x: start_x,
            player_y: start_y,
            player_render_x: start_x,
            player_render_y: start_y,
            player_dir: start_dir & 7,
            target: None,
            player_path: Vec::new(),
            player_step_t: 0.0,
            player_active: false,
            anim_time: 0.0,
            teleport_cooldown: 0.0,
            tables,
            settlements,
            population_cache: HashMap::new(),
            world_object_sources,
            active_world_objects: Vec::new(),
            world_frame: 0,
            world_frame_accum: 0.0,
            player_interact: false,
            pickup_message: None,
            pickup_message_t: 0.0,
            game_over: false,
            shadow_spans: 0,
            shadow_debug: Vec::new(),
            detail_pan: [0.0, 0.0],
            camera_follow: true,
            camera_follow_strength: 11.0,
            frame: vec![0u8; (VIRTUAL_W * VIRTUAL_H * 4) as usize],
            texts: Vec::new(),
            sounds: Vec::new(),
            leader_entity_index: 0,
            hovered_slot: None,
            character_screen: false,
            map_open: false,
            quest_log_open: false,
            hud_press: None,
            map_overlay: None,
            dialog: None,
            trade: None,
            trade_message: None,
            trade_message_t: 0.0,
            settlement_trade,
            host_w: VIRTUAL_W,
            host_h: VIRTUAL_H,
            drag: false,
            last_mouse: (0.0, 0.0),
            quit: false,
            request_menu: false,
            party_selected: [false; crate::hud::PARTY_CELLS],
            party: Vec::new(),
            quest_flags,
            discovered_locations: [0; 56],
            shift_down: false,
            ctrl_down: false,
            torch_reveal: false,
            dig_armed: false,
            inventory_owner: crate::inventory::InventoryOwner::Protagonist,
            held_item: None,
            inventory_scroll: 0,
            temp_effect_accum: 0.0,
            mouse_left_down: false,
            drag_start: None,
            dragging: false,
            party_step_t: 0.0,
            cursor_virtual: (-1.0, -1.0),
            combat_step_t: 0.0,
            combat_rng: crate::combat::OriginalRng::new(0x4B4F_4E55),
            sound_rng: crate::combat::OriginalRng::new(0x534F_554E),
            combat_frame: 0,
            player_attack_target: None,
            player_engaged: false,
            player_attack_frame: 0,
            player_attack_frames: 1,
            player_attack_ranged: false,
            player_ranged_frame: 0,
            player_ranged_frames: 0,
            player_ranged_fired: false,
            projectiles: Vec::new(),
            player_death_frames: 1,
            player_death_frame: 0,
            combat_message: None,
            combat_message_t: 0.0,
            combat_test_target: None,
            combat_test_prop: None,
            combat_attacks: 0,
            combat_hits: 0,
        };
        // The protagonist/leader (party slot 0) starts selected (`FUN_004246C0`
        // sets `selection[0] = leader` after loading the save).
        app.party_selected[0] = true;
        app.reset_object_group_activity();
        // New games start with the 56-byte mask cleared (`FUN_00436214` case 8
        // memsets `0x81CF80`); locations are revealed only by map items.
        app.sweep_all_owners();
        app.apply_settlement_props();
        app.rebuild_world_objects();
        app.center_camera_on_player(true, 0.0);
        app
    }

    /// Mark a location discovered in the original 56-byte world-map mask.
    pub fn mark_discovered(&mut self, loc_id: u32) {
        if (1..=56).contains(&loc_id) {
            self.discovered_locations[(loc_id - 1) as usize] = 1;
            self.map_overlay = None;
        }
    }

    // --- camera ---

    /// Advance the whole session by one rendered frame.
    ///
    /// Order matters and follows the original frame, not the browser refresh
    /// rate: clear the one-frame sound queue, advance the monotonic 60 ms
    /// original-frame clock (`world_frame`, which also drives the
    /// `(frame + 7) & 0xF == 0` settlement pass and the prop-flame tick), decay
    /// the timed HUD/pickup/trade messages, then run the gameplay passes
    /// (`tick_ambient_sound` -> population -> actor AI -> player -> party ->
    /// combat -> movie trigger -> temporary potion effects -> camera follow)
    /// and finally draw the world (`draw_detail`).
    ///
    /// `dt` is wall-clock seconds from the shell. Sounds queued below are
    /// drained by the shell before the next call, which is why the queue is
    /// cleared first.
    pub fn step(&mut self, dt: f64) {
        // One-frame sound queue: input events pushed since the last drain are
        // preserved by the shell before this call; sounds produced below are
        // drained on the next frame.
        self.sounds.clear();
        self.advance_world_frame(dt);
        if let Some((code, t)) = self.hud_press {
            let remaining = t - dt.max(0.0);
            self.hud_press = if remaining > 0.0 {
                Some((code, remaining))
            } else {
                None
            };
        }
        if self.pickup_message_t > 0.0 {
            self.pickup_message_t -= dt.max(0.0);
            if self.pickup_message_t <= 0.0 {
                self.pickup_message = None;
            }
        }
        if self.trade_message_t > 0.0 {
            self.trade_message_t -= dt.max(0.0);
            if self.trade_message_t <= 0.0 {
                self.trade_message = None;
            }
        }
        self.tick_ambient_sound();
        self.ensure_population(self.current_map);
        self.update_actors(dt);
        self.update_player(dt);
        self.update_party(dt);
        self.update_combat(dt);
        self.check_game_over_video();
        self.tick_temp_effects(dt);
        if self.camera_follow {
            self.center_camera_on_player(false, dt);
        }
        self.draw_detail();
    }
}

#[cfg(test)]
mod tests;
