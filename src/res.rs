//! Original resource decoding: GRAPH/OBJECTS archives, indexed RLE, palettes and sprite composition.
//!
//! `GRAPH.RES` layout as walked by `res/graph.rs::parse_graph_layout`:
//!
//! ```text
//! 0x00000..0x20000  256 palettes x 512 bytes (RGB555, palette id = offset / 512)
//! 0x20000           11 cursor blocks: [u32 tag][u32 size][16-bit color sprite]
//! after cursors     terrain block: [u32 tag][u32 size]
//!                     [512 x {u32 offset, u32 palette_offset}]  (0x1000 bytes)
//!                     tile payload region
//! after terrain     3 human base body blocks
//! after bases       24 human equipment overlay blocks
//! ```
//!
//! Every block begins with a `u32` tag and carries its byte size at `+4`. The
//! cursor/terrain walk is `pos += 8 + size`; for the human base and overlay
//! blocks the step is the size at `+4` alone (`pos += size`), as the parser
//! shows.
//!
//! `OBJECTS.RES`: an 8-byte header, a 512-entry `{u32 offset, u32 size}` index
//! at `+0x08`, and the block data region based at `0x1008`.
//!
//! Indexed sprite payloads use `raster.rs::decode_indexed_rle`:
//! `[u16 width][u16 height][u16 row_size[height]][rows]`, where in a row `0x00`
//! ends it, `0x80 | n` skips `n` transparent pixels and a literal token `n` is
//! followed by `n` palette indices. Colours are always taken from the 256-entry
//! palette selected by `palette_offset / 512`; the palette is per block, never
//! global.

use std::collections::HashMap;
use std::rc::Rc;

mod cursor;
mod exe;
mod flames;
mod graph;
mod human;
mod lights;
mod objects;
mod raster;
mod shadows;

pub use self::cursor::*;
pub use self::exe::*;
pub use self::flames::*;
use self::graph::parse_graph_layout;
pub use self::human::*;
pub use self::lights::*;
use self::objects::parse_object_index;
pub use self::raster::*;

pub use crate::constants::coords::MAP_COUNT;
pub use crate::constants::coords::MAP_H;
pub use crate::constants::coords::MAP_SIZE;
pub use crate::constants::coords::MAP_W;

pub use crate::constants::coords::GRID_ODD_SHIFT;
pub use crate::constants::coords::GRID_X_STEP;
pub use crate::constants::coords::GRID_Y_STEP;
pub use crate::constants::coords::MAP_ORIGIN_X;
pub use crate::constants::coords::MAP_PIXEL_H;
pub use crate::constants::coords::MAP_PIXEL_W;
pub use crate::constants::coords::TERRAIN_TILE_H;
pub use crate::constants::coords::TERRAIN_TILE_W;

pub use crate::constants::game_layout::GAME_ITEMS_OFF;
pub use crate::constants::game_layout::GAME_VARIANTS_OFF;
pub use crate::constants::game_layout::GAME_VARIANT_COUNT;
pub use crate::constants::game_layout::GAME_VARIANT_SIZE;

impl Image {
    /// Allocate `w * h` RGBA pixels and fill them with `fill`.
    ///
    /// An all-zero `fill` leaves the buffer zeroed, so transparent surfaces cost
    /// no per-pixel stores.
    pub fn new(w: usize, h: usize, fill: [u8; 4]) -> Image {
        let mut data = vec![0u8; w * h * 4];
        if fill[3] != 0 || fill[0] != 0 || fill[1] != 0 || fill[2] != 0 {
            let mut i = 0;
            while i < data.len() {
                data[i..i + 4].copy_from_slice(&fill);
                i += 4;
            }
        }
        Image { w, h, data }
    }
}

/// The decoded original resource set plus its address profile and sprite caches.
///
/// One `Res` owns the four raw archives, the palette table and the parsed block
/// layout; every sprite accessor decodes lazily through the caches below.
pub struct Res {
    /// `Game.res` bytes. Only the variant table at `GAME_VARIANTS_OFF` is read
    /// from here: it is **not** the item-instance table of a loaded game.
    pub game: Vec<u8>,
    /// The loaded save file (`Game.0/1/2`). Item instances are read from here
    /// (offset 0, 8192 × 16), not from `game` (which is `Game.res` and only
    /// supplies the variant table).
    pub save_game: Vec<u8>,
    /// `GRAPH.RES` bytes: palettes, cursor blocks, terrain, human blocks.
    pub graph: Vec<u8>,
    /// `Objects.res` bytes: the object/monster block archive.
    pub objects: Vec<u8>,
    /// The shipped executable image, whose DGROUP holds the recovered tables.
    pub exe: Vec<u8>,
    /// VA-to-file mapping for this executable size (see `exe_profile`).
    pub profile: ExeProfile,
    /// The 256 RGB555 palettes of `GRAPH.RES`, expanded to RGB; index
    /// `palette_offset / 512`.
    pub palettes: Vec<[[u8; 3]; 256]>,

    /// `GRAPH.RES` offset of the 512-entry terrain table
    /// (`{u32 offset, u32 palette_offset}` records).
    pub terrain_table_pos: usize,
    /// `GRAPH.RES` offset of the terrain tile payload region.
    pub terrain_payload_pos: usize,
    /// Byte length of the terrain tile payload region.
    pub terrain_payload_size: usize,

    /// `GRAPH.RES` offsets of the three human base body blocks.
    pub human_base_positions: Vec<usize>,
    /// `GRAPH.RES` offsets of the 24 human equipment overlay blocks (indexed by
    /// the item prototype's `equipped_visual`).
    pub human_overlay_positions: Vec<usize>,
    /// `OBJECTS.RES` block index: `(offset, size)` per object id, `None` for
    /// unused/out-of-range slots.
    pub object_entries: Vec<Option<(usize, usize)>>,
    /// Palette offset of each of the 24 human overlay blocks, read as 24
    /// little-endian dwords at `ExeProfile::overlay_palette_va`.
    pub overlay_palette_offsets: Vec<u32>,

    /// Decoded `INTR{width}R.RES` interface archive (attached at game start via
    /// [`Res::load_intr`]); `None` until/unless loaded. Not yet consumed by the
    /// renderer.
    pub intr: Option<crate::intr::IntrArchive>,
    /// Decoded `QUEST_R.RES` dialogue/quest database (attached at game start via
    /// [`Res::load_quest`]); `None` until/unless loaded.
    pub quest: Option<crate::quest::QuestData>,
    /// Item template database parsed from `konung.res` (icons + CP866 names).
    pub item_db: Option<crate::item::ItemDb>,
    /// Original bitmap fonts parsed from `konung.res` (used for item labels).
    pub fonts: crate::font::Fonts,
    /// Raw `konung.res` (character/class name tables). Empty until loaded.
    pub konung: Vec<u8>,
    /// Decoded `FLAMES.RES` fire frames (74), empty until loaded.
    pub flames: Vec<Img>,
    /// Decoded `LIGHTS.RES` masks/tiles, `None` until loaded.
    pub lights: Option<LightRes>,

    /// Decoded terrain tiles keyed by the 9-bit tile id (see `graph.rs`).
    terrain_tile_cache: HashMap<usize, Option<Img>>,
    /// Decoded prop sprites keyed by `(object_id, palette_override)` together
    /// with the sprite anchor `(gx, gy)` of frame 0.
    prop_sprite_cache: HashMap<(usize, u32), (Img, i32, i32)>,
    /// Decoded monster/object sprite image keyed by `(object_id, palette_id,
    /// frame_id)`. The per-frame screen anchor (gx/gy) is **not** cached here:
    /// it depends on `(row, direction, frame_index)` and is resolved separately
    /// in `objects.rs::monster_anchor`.
    monster_frame_cache: HashMap<(usize, u32, usize), Img>,
    /// Composed NPC/protagonist human frames keyed by
    /// `(base_or_variant, action, direction, frame_index, palette, battle,
    /// weapon_slot, equipment)`, with the `(image, gx, gy, count)` result.
    npc_frame_cache: HashMap<(i32, i32, i32, i32, i32, bool, u8, [u16; 6]), (Img, i32, i32, i32)>,
    /// Composed 160x160 character-sheet paper dolls keyed by the same pose and
    /// loadout fields as the world frame.
    paper_doll_cache: HashMap<(usize, i32, bool, u8, usize, usize, usize, [u16; 6]), Img>,
}

impl Res {
    /// Build a resource set from the four raw archives and parse their layouts.
    ///
    /// # Panics
    ///
    /// Panics when `exe` is not one of the two supported executable images,
    /// because every DGROUP table address depends on the profile.
    pub fn new(game: Vec<u8>, graph: Vec<u8>, objects: Vec<u8>, exe: Vec<u8>) -> Res {
        let profile = exe_profile(exe.len())
            .unwrap_or_else(|| panic!("unsupported executable size {}", exe.len()));
        let palettes: Vec<[[u8; 3]; 256]> = (0..256).map(|i| palette_rgb(&graph, i)).collect();

        let (
            terrain_table_pos,
            terrain_payload_pos,
            terrain_payload_size,
            human_base_positions,
            human_overlay_positions,
        ) = parse_graph_layout(&graph);

        let object_entries = parse_object_index(&objects);

        let pal_off = exe_va_to_off(profile.overlay_palette_va, &profile);
        let mut overlay_palette_offsets = Vec::with_capacity(24);
        for i in 0..24 {
            let o = pal_off + i * 4;
            overlay_palette_offsets.push(if o + 4 <= exe.len() { u32(&exe, o) } else { 0 });
        }

        Res {
            game,
            save_game: Vec::new(),
            graph,
            objects,
            exe,
            profile,
            palettes,
            terrain_table_pos,
            terrain_payload_pos,
            terrain_payload_size,
            human_base_positions,
            human_overlay_positions,
            object_entries,
            overlay_palette_offsets,
            intr: None,
            quest: None,
            item_db: None,
            fonts: crate::font::Fonts::empty(),
            konung: Vec::new(),
            flames: Vec::new(),
            lights: None,
            terrain_tile_cache: HashMap::new(),
            prop_sprite_cache: HashMap::new(),
            monster_frame_cache: HashMap::new(),
            npc_frame_cache: HashMap::new(),
            paper_doll_cache: HashMap::new(),
        }
    }

    /// Return the four large original resource buffers and drop the decoded
    /// caches. Used by the frontend to compose one-time hero previews without
    /// cloning GRAPH/OBJECTS, and by the host lifecycle to reclaim the buffers
    /// when a live game is discarded (see `World::into_loader_parts`).
    pub fn into_parts(self) -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) {
        (self.game, self.graph, self.objects, self.exe)
    }

    /// One 48-byte `Game.res` variant record (`GAME_VARIANT_SIZE`) selected by
    /// its table index.
    ///
    /// Returns an empty slice when `variant_id` points past the buffer, so
    /// callers must bound-check the fields they read.
    pub fn variant(&self, variant_id: usize) -> &[u8] {
        let o = GAME_VARIANTS_OFF + variant_id * GAME_VARIANT_SIZE;
        &self.game[o..o + GAME_VARIANT_SIZE]
    }

    /// Bytes of one `OBJECTS.RES` block by object id, or an empty slice for an
    /// unused index or a record that runs past the archive.
    pub fn object_block(&self, object_id: usize) -> &[u8] {
        match self.object_entries.get(object_id).and_then(|x| *x) {
            Some((off, size)) if off + size <= self.objects.len() => &self.objects[off..off + size],
            _ => &[],
        }
    }

    // --- INTR interface sprites (original UI asset family) ---

    /// Decode and attach an `INTR{width}R.RES` archive using this resource's
    /// already-loaded GRAPH palettes. Validates the whole archive up front;
    /// pixel buffers are decoded lazily per index. Returns the number of
    /// occupied sprite indices.
    pub fn load_intr(&mut self, intr_data: &[u8]) -> Result<usize, crate::intr::IntrError> {
        let archive = crate::intr::IntrArchive::load(intr_data, &self.graph)?;
        let count = archive.len();
        self.intr = Some(archive);
        Ok(count)
    }

    /// Decoded INTR interface sprite by its original numeric index, or `None`
    /// when no archive is attached or the slot is unused.
    pub fn intr_sprite(&mut self, id: usize) -> Option<Img> {
        self.intr.as_mut().and_then(|a| a.get(id))
    }

    // --- dialogue / quest database (`QUEST_R.RES`) ---

    /// Parse and attach the `QUEST_R.RES` dialogue/quest database. Rejects a
    /// truncated image (never a partially valid database).
    pub fn load_quest(&mut self, quest_data: &[u8]) -> Result<(), crate::quest::QuestError> {
        self.quest = Some(crate::quest::QuestData::parse(quest_data)?);
        Ok(())
    }

    // --- items (loaded Game.N instances + konung.res templates) ---

    /// Attach the loaded save file (`Game.0/1/2`) whose instance table at offset
    /// 0 resolves the protagonist's inventory/equipment ids.
    pub fn set_save_game(&mut self, save: &[u8]) {
        self.save_game = save.to_vec();
    }

    /// Authored `Entity+0xD7` byte from the loaded `Game.N` table. The M/O
    /// walk-decision delay uses it as the random delay ceiling (`FUN_004123BC`).
    pub fn entity_raw_d7(&self, id: usize) -> u8 {
        let off = 0x2A69A_usize
            .checked_add(id.saturating_mul(0x100))
            .and_then(|o| o.checked_add(0xD7));
        off.and_then(|o| self.save_game.get(o).copied())
            .unwrap_or(0)
    }

    /// Parse the original item templates and bitmap fonts from `konung.res`
    /// (the `KIND_NAMES` resource). Idempotent and safe to skip when absent.
    pub fn load_items_and_fonts(&mut self, konung: &[u8]) {
        if konung.is_empty() {
            return;
        }
        self.item_db = Some(crate::item::ItemDb::from_konung(konung));
        self.fonts = crate::font::Fonts::parse(konung);
        self.konung = konung.to_vec();
    }

    /// Original character name by `Entity+0xD4` from `konung.res`.
    pub fn character_name(&self, name_id: u8) -> Option<String> {
        if self.konung.is_empty() {
            return None;
        }
        crate::names::character_name(&self.konung, name_id)
    }

    /// Original class name by `Entity+0x04` from `konung.res`.
    pub fn class_name(&self, entity_class: u8) -> Option<String> {
        if self.konung.is_empty() {
            return None;
        }
        crate::names::class_name(&self.konung, entity_class)
    }

    /// Original character nickname/surname by `Entity+0xD5` from `konung.res`.
    pub fn nickname(&self, nickname_id: u8) -> Option<String> {
        if self.konung.is_empty() {
            return None;
        }
        crate::names::nickname(&self.konung, nickname_id)
    }

    /// Original monster name by `Entity+0x04 & 0x7F` from `konung.res`.
    pub fn monster_name(&self, archetype_low: u8) -> Option<String> {
        if self.konung.is_empty() {
            return None;
        }
        crate::names::monster_name(&self.konung, archetype_low)
    }

    /// Settlement role name by `FUN_0041182C` slot `1..=5` from `konung.res`.
    pub fn profession_name(&self, role: u8) -> Option<String> {
        if self.konung.is_empty() {
            return None;
        }
        crate::names::profession_name(&self.konung, role)
    }

    /// Resolve an item-instance id (from an `Entity` inventory/equipment field)
    /// to its original icon index and CP866 name. The instance is decoded from
    /// the loaded save file (`Game.N`), matching the original loader. `None`
    /// for the empty id `0`, when the save has not been attached, or when the
    /// item database is not loaded.
    pub fn item_info(&self, id: u16) -> Option<crate::item::ItemInfo> {
        self.item_db.as_ref()?.resolve(&self.save_game, id)
    }

    /// Full lower-panel item detail string, porting `FUN_0042F5F8`.
    ///
    /// Builds the same single-line CP866-derived text the original draws in the
    /// bottom panel: name, combat/armor rating, wear, weight, poison, weapon
    /// modifier, quality-condition bonuses/requirements and prototype
    /// requirement. Returns `None` for id `0`, a missing save, or an unresolved
    /// template.
    pub fn item_hover_detail(&self, id: u16) -> Option<String> {
        let db = self.item_db.as_ref()?;
        let inst = crate::item::ItemDb::decode_instance(&self.save_game, id)?;
        db.hover_detail(&inst)
    }

    /// Template info by class/subtype (for generated merchant stock).
    pub fn item_info_for(&self, class: u8, subtype: u8) -> Option<crate::item::ItemInfo> {
        self.item_db.as_ref()?.info(class, subtype)
    }

    /// Clone a 16-byte item instance into the first free instance slot.
    ///
    /// Mirrors `FUN_00438884`: scan from instance 1 for a record whose signed
    /// class byte is negative (the shipped `Game.N` tables use `0xFF`), then
    /// copy all 16 bytes. Returns the new instance id.
    pub fn alloc_item_like(&mut self, source: u16) -> Option<u16> {
        if source == 0 || self.save_game.len() < 8192 * crate::item::ITEM_INSTANCE_SIZE {
            return None;
        }
        let src_off = source as usize * crate::item::ITEM_INSTANCE_SIZE;
        if src_off + crate::item::ITEM_INSTANCE_SIZE > self.save_game.len() {
            return None;
        }
        let mut item = [0u8; 16];
        item.copy_from_slice(&self.save_game[src_off..src_off + 16]);
        for id in 1..8192usize {
            let o = id * crate::item::ITEM_INSTANCE_SIZE;
            if (self.save_game[o] as i8) < 0 {
                self.save_game[o..o + 16].copy_from_slice(&item);
                return Some(id as u16);
            }
        }
        None
    }

    /// Full decoded 16-byte item instance, or `None` for id `0`, a missing save,
    /// or an out-of-range id.
    pub fn item_instance(&self, id: u16) -> Option<crate::item::ItemInstance> {
        crate::item::ItemDb::decode_instance(&self.save_game, id)
    }

    /// `(current, max)` durability of item instance `id`.
    pub fn item_durability(&self, id: u16) -> Option<(f32, f32)> {
        let i = self.item_instance(id)?;
        Some((i.current_durability, i.max_durability))
    }

    /// `(class, subtype)` of an item instance.
    pub fn item_class_subtype(&self, id: u16) -> Option<(u8, u8)> {
        let i = self.item_instance(id)?;
        Some((i.class, i.subtype))
    }

    /// Original trade value of item instance `id` (`FUN_0041B2A8`).
    pub fn item_trade_value(&self, id: u16) -> Option<i32> {
        let db = self.item_db.as_ref()?;
        db.instance_trade_value_by_id(&self.save_game, id)
    }

    /// `class` byte of an item instance.
    pub fn item_class(&self, id: u16) -> Option<u8> {
        self.item_instance(id).map(|i| i.class)
    }

    /// Write the instance `class` byte. Used by the broken/consumed paths
    /// (`class = 0xFF`) and the craft result transformation.
    pub fn set_item_class(&mut self, id: u16, class: u8) -> bool {
        if id == 0 {
            return false;
        }
        let o = id as usize * crate::item::ITEM_INSTANCE_SIZE;
        if o + 16 > self.save_game.len() {
            return false;
        }
        self.save_game[o] = class;
        true
    }

    /// Write the instance `subtype` byte (`+0x03`).
    pub fn set_item_subtype(&mut self, id: u16, subtype: u8) -> bool {
        if id == 0 {
            return false;
        }
        let o = id as usize * crate::item::ITEM_INSTANCE_SIZE;
        if o + 16 > self.save_game.len() {
            return false;
        }
        self.save_game[o + 3] = subtype;
        true
    }

    /// Write the instance `condition` byte (`+0x01`).
    pub fn set_item_condition(&mut self, id: u16, condition: i8) -> bool {
        if id == 0 {
            return false;
        }
        let o = id as usize * crate::item::ITEM_INSTANCE_SIZE;
        if o + 16 > self.save_game.len() {
            return false;
        }
        self.save_game[o + 1] = condition as u8;
        true
    }

    /// Copy the complete 16-byte instance record `src` onto `dst`
    /// (`FUN_00414DE4` case 7, «Дупликатор»).
    pub fn copy_item_instance(&mut self, src: u16, dst: u16) -> bool {
        if src == 0 || dst == 0 || src == dst {
            return false;
        }
        let so = src as usize * crate::item::ITEM_INSTANCE_SIZE;
        let dso = dst as usize * crate::item::ITEM_INSTANCE_SIZE;
        if so + 16 > self.save_game.len() || dso + 16 > self.save_game.len() {
            return false;
        }
        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(&self.save_game[so..so + 16]);
        self.save_game[dso..dso + 16].copy_from_slice(&bytes);
        true
    }

    /// Current quality / durability value (`+0x04 f32`).
    pub fn item_quality(&self, id: u16) -> Option<f32> {
        self.item_instance(id).map(|i| i.current_durability)
    }

    /// Write only the current quality / durability value (`+0x04 f32`).
    pub fn set_item_quality(&mut self, id: u16, quality: f32) -> bool {
        if id == 0 || !quality.is_finite() {
            return false;
        }
        let o = id as usize * crate::item::ITEM_INSTANCE_SIZE;
        if o + 8 > self.save_game.len() {
            return false;
        }
        self.save_game[o + 4..o + 8].copy_from_slice(&quality.to_le_bytes());
        true
    }

    /// Stack-compatibility key: `(class, condition, modifier, subtype, poison,
    /// flags)`. Two class-12 instances with the same key may be merged.
    pub fn item_stack_key(&self, id: u16) -> Option<(u8, i8, i8, u8, u16, u16)> {
        let i = self.item_instance(id)?;
        Some((
            i.class,
            i.condition,
            i.modifier_index,
            i.subtype,
            i.poison,
            i.flags,
        ))
    }

    /// Add to a class-12 stack quantity (`+0x04 u32`) and return the new value.
    pub fn add_item_quantity(&mut self, id: u16, add: u32) -> Option<u32> {
        if id == 0 || add == 0 {
            return None;
        }
        let o = id as usize * crate::item::ITEM_INSTANCE_SIZE;
        if o + 8 > self.save_game.len() {
            return None;
        }
        let q = crate::res::u32(&self.save_game, o + 4).saturating_add(add);
        self.save_game[o + 4..o + 8].copy_from_slice(&q.to_le_bytes());
        Some(q)
    }

    /// Subtract `loss` from the current durability and mark the instance broken
    /// (`class = 0xFF`) when it reaches zero. Returns `true` when it broke.
    /// Mirrors the durability writes in `FUN_0041c3cc`.
    pub fn damage_item_durability(&mut self, id: u16, loss: f32) -> bool {
        if id == 0 || !loss.is_finite() || loss <= 0.0 {
            return false;
        }
        let Some(inst) = self.item_instance(id) else {
            return false;
        };
        let o = id as usize * crate::item::ITEM_INSTANCE_SIZE;
        let current = inst.current_durability - loss;
        self.save_game[o + 4..o + 8].copy_from_slice(&current.to_le_bytes());
        if current <= 0.0 {
            self.save_game[o] = 0xFF;
            true
        } else {
            false
        }
    }

    /// Stack quantity of a class-12 arrow/ammo instance (`+0x04 u32`).
    pub fn item_quantity(&self, id: u16) -> Option<u32> {
        if id == 0 {
            return None;
        }
        let o = id as usize * crate::item::ITEM_INSTANCE_SIZE;
        if o + 4 > self.save_game.len() {
            return None;
        }
        Some(crate::res::u32(&self.save_game, o + 4))
    }

    /// Decrement an ammo stack by one (`FUN_004123BC` action row 4). When the
    /// stack reaches zero the instance class becomes `0xFF`. Returns the
    /// remaining quantity.
    pub fn decrement_item_quantity(&mut self, id: u16) -> Option<u32> {
        if id == 0 {
            return None;
        }
        let o = id as usize * crate::item::ITEM_INSTANCE_SIZE;
        if o + 16 > self.save_game.len() {
            return None;
        }
        let q = crate::res::u32(&self.save_game, o + 4).saturating_sub(1);
        self.save_game[o + 4..o + 8].copy_from_slice(&q.to_le_bytes());
        if q == 0 {
            self.save_game[o] = 0xFF;
        }
        Some(q)
    }

    /// Diagnostic/test helper: set an ammo stack quantity (`+0x04 u32`).
    pub fn set_item_quantity(&mut self, id: u16, quantity: u32) -> bool {
        if id == 0 {
            return false;
        }
        let o = id as usize * crate::item::ITEM_INSTANCE_SIZE;
        if o + 8 > self.save_game.len() {
            return false;
        }
        self.save_game[o + 4..o + 8].copy_from_slice(&quantity.to_le_bytes());
        if quantity > 0 {
            self.save_game[o] = 12;
        }
        true
    }

    /// Overwrite an instance's `+0x02` modifier byte. Used by the target-item
    /// use path (`FUN_0041DAB0` case 7: oil turns arrows into plain ammo).
    pub fn set_item_modifier(&mut self, id: u16, modifier: i8) -> bool {
        if id == 0 {
            return false;
        }
        let o = id as usize * crate::item::ITEM_INSTANCE_SIZE;
        if o + 16 > self.save_game.len() {
            return false;
        }
        self.save_game[o + 2] = modifier as u8;
        true
    }

    /// Overwrite an instance's `+0x0E` flags word. Bit 0 is the original
    /// **unidentified** marker toggled by `FUN_0041BA08` (and set by generated
    /// loot); the trade-value condition bonus only applies when it is clear.
    pub fn set_item_flags(&mut self, id: u16, flags: u16) -> bool {
        if id == 0 {
            return false;
        }
        let o = id as usize * crate::item::ITEM_INSTANCE_SIZE;
        if o + 16 > self.save_game.len() {
            return false;
        }
        self.save_game[o + 0x0E..o + 0x10].copy_from_slice(&flags.to_le_bytes());
        true
    }

    /// Overwrite an instance's `+0x0C` poison/charge value
    /// (`FUN_0041DAB0` case 6: Яд on a weapon or arrow stack).
    pub fn set_item_poison(&mut self, id: u16, poison: u16) -> bool {
        if id == 0 {
            return false;
        }
        let o = id as usize * crate::item::ITEM_INSTANCE_SIZE;
        if o + 16 > self.save_game.len() {
            return false;
        }
        self.save_game[o + 0x0C..o + 0x0E].copy_from_slice(&poison.to_le_bytes());
        true
    }

    /// Diagnostic/test helper: set current durability and max durability.
    pub fn set_item_durability(&mut self, id: u16, current: f32) -> bool {
        if id == 0 {
            return false;
        }
        let o = id as usize * crate::item::ITEM_INSTANCE_SIZE;
        if o + 12 > self.save_game.len() {
            return false;
        }
        self.save_game[o + 4..o + 8].copy_from_slice(&current.to_le_bytes());
        self.save_game[o + 8..o + 12].copy_from_slice(&current.to_le_bytes());
        true
    }

    /// Decrement the instance `+0x0C` counter used by the weapon wear path
    /// (`FUN_0041c3cc`); returns the remaining value.
    pub fn decrement_item_charge(&mut self, id: u16) -> Option<u16> {
        if id == 0 {
            return None;
        }
        let o = id as usize * crate::item::ITEM_INSTANCE_SIZE;
        if o + 0x10 > self.save_game.len() {
            return None;
        }
        let v = crate::res::u16(&self.save_game, o + 0x0C) as u16;
        let next = v.saturating_sub(1);
        self.save_game[o + 0x0C..o + 0x0E].copy_from_slice(&next.to_le_bytes());
        Some(next)
    }

    /// Allocate a fresh item instance in the first free slot (`FUN_00438884`
    /// shape) and fill the verified instance fields.
    #[allow(clippy::too_many_arguments)]
    pub fn alloc_item_instance(
        &mut self,
        class: u8,
        subtype: u8,
        condition: i8,
        modifier_index: i8,
        durability: f32,
        poison: u16,
        flags: u16,
    ) -> Option<u16> {
        if self.save_game.len() < 8192 * crate::item::ITEM_INSTANCE_SIZE {
            return None;
        }
        for id in 1..8192usize {
            let o = id * crate::item::ITEM_INSTANCE_SIZE;
            if (self.save_game[o] as i8) < 0 {
                let rec = &mut self.save_game[o..o + 16];
                rec.fill(0);
                rec[0] = class;
                rec[1] = condition as u8;
                rec[2] = modifier_index as u8;
                rec[3] = subtype;
                rec[4..8].copy_from_slice(&durability.to_le_bytes());
                rec[8..12].copy_from_slice(&durability.to_le_bytes());
                rec[12..14].copy_from_slice(&poison.to_le_bytes());
                rec[14..16].copy_from_slice(&flags.to_le_bytes());
                return Some(id as u16);
            }
        }
        None
    }

    // --- terrain ---
}
#[cfg(test)]
mod tests;
