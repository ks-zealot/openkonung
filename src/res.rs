use std::collections::HashMap;
use std::rc::Rc;


mod exe;
mod graph;
mod human;
mod objects;
mod raster;

pub use self::exe::*;
pub use self::human::*;
pub use self::raster::*;
use self::graph::parse_graph_layout;
use self::objects::parse_object_index;

pub use crate::constants::coords::MAP_COUNT;
pub use crate::constants::coords::MAP_SIZE;
pub use crate::constants::coords::MAP_W;
pub use crate::constants::coords::MAP_H;

pub use crate::constants::coords::GRID_X_STEP;
pub use crate::constants::coords::GRID_Y_STEP;
pub use crate::constants::coords::GRID_ODD_SHIFT;
pub use crate::constants::coords::TERRAIN_TILE_W;
pub use crate::constants::coords::TERRAIN_TILE_H;
pub use crate::constants::coords::MAP_ORIGIN_X;
pub use crate::constants::coords::MAP_PIXEL_W;
pub use crate::constants::coords::MAP_PIXEL_H;

pub use crate::constants::game_layout::GAME_ITEMS_OFF;
pub use crate::constants::game_layout::GAME_VARIANTS_OFF;
pub use crate::constants::game_layout::GAME_VARIANT_SIZE;
pub use crate::constants::game_layout::GAME_VARIANT_COUNT;

impl Image {
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

pub struct Res {
    pub game: Vec<u8>,
    /// The loaded save file (`Game.0/1/2`). Item instances are read from here
    /// (offset 0, 8192 × 16), not from `game` (which is `Game.res` and only
    /// supplies the variant table).
    pub save_game: Vec<u8>,
    pub graph: Vec<u8>,
    pub objects: Vec<u8>,
    pub exe: Vec<u8>,
    pub profile: ExeProfile,
    pub palettes: Vec<[[u8; 3]; 256]>,

    pub terrain_table_pos: usize,
    pub terrain_payload_pos: usize,
    pub terrain_payload_size: usize,

    pub human_base_positions: Vec<usize>,
    pub human_overlay_positions: Vec<usize>,
    pub object_entries: Vec<Option<(usize, usize)>>,
    pub overlay_palette_offsets: Vec<u32>,

    /// Decoded `INTR{width}R.RES` interface archive (attached at game start via
    /// [`Res::load_intr`]); `None` until/unless loaded. Not yet consumed by the
    /// renderer.
    pub intr: Option<crate::intr::IntrArchive>,
    /// Item template database parsed from `konung.res` (icons + CP866 names).
    pub item_db: Option<crate::item::ItemDb>,
    /// Original bitmap fonts parsed from `konung.res` (used for item labels).
    pub fonts: crate::font::Fonts,
    /// Raw `konung.res` (character/class name tables). Empty until loaded.
    pub konung: Vec<u8>,

    terrain_tile_cache: HashMap<usize, Option<Img>>,
    prop_sprite_cache: HashMap<(usize, u32), (Img, i32, i32)>,
    monster_frame_cache: HashMap<(usize, u32, usize), (Img, i32, i32, usize)>,
    npc_frame_cache: HashMap<(i32, i32, i32, i32, i32, [u16; 6]), (Img, i32, i32, i32)>,
}

impl Res {
    pub fn new(game: Vec<u8>, graph: Vec<u8>, objects: Vec<u8>, exe: Vec<u8>) -> Res {
        let profile = exe_profile(exe.len())
            .unwrap_or_else(|| panic!("unsupported executable size {}", exe.len()));
        let palettes: Vec<[[u8; 3]; 256]> = (0..256).map(|i| palette_rgb(&graph, i)).collect();

        let (terrain_table_pos, terrain_payload_pos, terrain_payload_size, human_base_positions, human_overlay_positions) =
            parse_graph_layout(&graph);

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
            item_db: None,
            fonts: crate::font::Fonts::empty(),
            konung: Vec::new(),
            terrain_tile_cache: HashMap::new(),
            prop_sprite_cache: HashMap::new(),
            monster_frame_cache: HashMap::new(),
            npc_frame_cache: HashMap::new(),
        }
    }


    pub fn variant(&self, variant_id: usize) -> &[u8] {
        let o = GAME_VARIANTS_OFF + variant_id * GAME_VARIANT_SIZE;
        &self.game[o..o + GAME_VARIANT_SIZE]
    }


    pub fn object_block(&self, object_id: usize) -> &[u8] {
        match self.object_entries.get(object_id).and_then(|x| *x) {
            Some((off, size)) if off + size <= self.objects.len() => {
                &self.objects[off..off + size]
            }
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

    // --- items (loaded Game.N instances + konung.res templates) ---


    /// Attach the loaded save file (`Game.0/1/2`) whose instance table at offset
    /// 0 resolves the protagonist's inventory/equipment ids.
    pub fn set_save_game(&mut self, save: &[u8]) {
        self.save_game = save.to_vec();
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


    /// Resolve an item-instance id (from an `Entity` inventory/equipment field)
    /// to its original icon index and CP866 name. The instance is decoded from
    /// the loaded save file (`Game.N`), matching the original loader. `None`
    /// for the empty id `0`, when the save has not been attached, or when the
    /// item database is not loaded.
    pub fn item_info(&self, id: u16) -> Option<crate::item::ItemInfo> {
        self.item_db.as_ref()?.resolve(&self.save_game, id)
    }

    // --- terrain ---


}
#[cfg(test)]
mod tests;
