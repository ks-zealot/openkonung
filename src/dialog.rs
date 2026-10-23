//! Original Konung **dialogue interface**: state machine + renderer.
//!
//! Recovered from `konung.res` (`DIALOG_SYSTEM.md`):
//!
//! - Entered by clicking an NPC whose `Entity+0xD0` (`dialogue_id`) is not
//!   `0xFF` (`FUN_0042260C` -> `FUN_00435290`). The id indexes `QUEST_R.RES`
//!   block B2 for the first node.
//! - The interface is drawn by `FUN_0042CBD0(0)` (case `param+1 == 1`): it first
//!   tiles **INTR[5]** (a 100x100 stone tile, `FUN_00430430`) over the window
//!   rect as the background, then blits **INTR[1]** (the ~3%-opaque ornate
//!   border) and **INTR[0]** (the divider band) at the per-resolution table
//!   `0x4632C4..0x4632DC`, then draws the NPC text and the option list with the
//!   small font (`0x58BC34`); the selected option uses the highlight font
//!   (`0x58B8F0`). The text drawer `FUN_0042BD8C` wraps to a max width and
//!   advances **15 px** per line.
//! - A node's entries are `conditional` (f7) **NPC statement lines** followed by
//!   `unconditional` (f6) **selectable options** (`FUN_00434C74`): the first list
//!   (`0x788FB4`) is drawn as text, the second (`0x788DB4`) as the choices.
//! - Selecting an option follows its `+0x04` node link; `-1`/`-2` end the dialog.
//!
//! Conditions and action programs (including trade) are intentionally **not**
//! executed yet: every entry is shown and every action is a no-op.

use crate::font::{Fonts, OriginalFont};
use crate::quest::{QuestData, LINK_END, LINK_NONE};
use crate::res::{blit, Res};

/// INTR index of the dialog window background (`0x61F3E0` = entry[1]).
pub const WINDOW_SPRITE: usize = 1;
/// INTR index of the dialog band/divider (`0x61F3D8` = entry[0]).
pub const BAND_SPRITE: usize = 0;
/// INTR index of the 100x100 stone tile (`0x61F400` = entry[5]) tiled by the
/// original `FUN_00430430` to fill the window behind the border.
pub const TILE_SPRITE: usize = 5;
/// Edge length of [`TILE_SPRITE`]; the original tiles in 100-px steps.
pub const TILE: i32 = 100;
/// Per-line advance used by `FUN_0042BD8C` (`0xF`).
pub const LINE_H: i32 = 15;
/// Maximum node hops when resolving branch nodes (safety bound).
const MAX_RESOLVE_HOPS: usize = 64;

/// Per-resolution dialog layout (tables `0x4632C4..0x46330C`; selector
/// `_DAT_0081D644`: 0 = 640, 1 = 800).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DialogLayout {
    /// INTR[1] window position.
    pub window_pos: (i32, i32),
    /// INTR[0] band position.
    pub band_pos: (i32, i32),
    /// NPC statement column origin.
    pub text_pos: (i32, i32),
    /// Wrap width shared by the statement and option columns.
    pub text_width: i32,
    /// Selectable-option column origin.
    pub opt_pos: (i32, i32),
}

pub fn layout(logical_w: i32) -> DialogLayout {
    if logical_w <= 640 {
        DialogLayout {
            window_pos: (114, 0),
            band_pos: (129, 150),
            text_pos: (133, 20),
            text_width: 485,
            opt_pos: (133, 186),
        }
    } else {
        DialogLayout {
            window_pos: (144, 0),
            band_pos: (161, 187),
            text_pos: (166, 25),
            text_width: 606,
            opt_pos: (166, 232),
        }
    }
}

/// One selectable option (an unconditional page entry).
#[derive(Clone, Debug)]
pub struct DialogOption {
    /// Wrapped rendered lines (small font).
    pub lines: Vec<String>,
    /// `+0x04` next node link (`-1` fall back, `-2` end).
    pub next: i16,
    /// `B1` record index of the option.
    pub record: usize,
    /// `+0x0A` `B5` action program id (`-1` = none); run on selection
    /// (`FUN_00434BB8`).
    pub action: i16,
    /// Bounding rect `(x, y, w, h)` in virtual coordinates.
    pub row: (i32, i32, i32, i32),
}

impl DialogOption {
    pub fn label(&self) -> String {
        self.lines.join(" ")
    }
}

/// Live dialogue state.
#[derive(Clone, Debug)]
pub struct DialogState {
    /// Source `Entity` id of the NPC being talked to.
    pub npc_entity: usize,
    /// Current `B1` display page index.
    pub page: usize,
    /// NPC statement lines (already wrapped).
    pub prompt_lines: Vec<String>,
    pub options: Vec<DialogOption>,
    /// Currently highlighted option (mouse hover), if any.
    pub selected: Option<usize>,
    /// `B5` action program ids of the included conditional (statement) entries;
    /// the original `FUN_00434C74` runs them when the page is built.
    pub on_open_actions: Vec<u16>,
}

/// Greedy wrap to `max_px` using the original small-font advances.
fn wrap(fonts: &Fonts, text: &str, max_px: i32) -> Vec<String> {
    let max_px = max_px.max(16);
    let measure = |s: &str| fonts.measure(OriginalFont::Small, s);
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in text.split(' ') {
        if cur.is_empty() {
            cur.push_str(word);
            continue;
        }
        let trial = format!("{} {}", cur, word);
        if measure(&trial) <= max_px {
            cur = trial;
        } else {
            lines.push(std::mem::take(&mut cur));
            cur.push_str(word);
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn contains(r: (i32, i32, i32, i32), x: i32, y: i32) -> bool {
    x >= r.0 && x < r.0 + r.2 && y >= r.1 && y < r.1 + r.3
}

/// Tile origins covering `rect = (x, y, w, h)` in `tile`-sized steps, from the
/// top-left (the original `FUN_00430430` loop order). Each origin is a tile
/// blit position; the last row/column may overhang and is clipped by the caller.
pub fn tile_origins(rect: (i32, i32, i32, i32), tile: i32) -> Vec<(i32, i32)> {
    let (x, y, w, h) = rect;
    let tile = tile.max(1);
    let cols = (w + tile - 1) / tile;
    let rows = (h + tile - 1) / tile;
    let mut out = Vec::with_capacity((rows * cols).max(0) as usize);
    for r in 0..rows {
        for c in 0..cols {
            out.push((x + c * tile, y + r * tile));
        }
    }
    out
}

/// Blit `img` at `(x, y)` but only the pixels inside the `clip` rect
/// `(cx, cy, cw, ch)` — the original tiles the stone background with a clip
/// (`FUN_00427698`), so a partial tile must not spill past the window edge.
fn blit_clip(
    dst: &mut [u8],
    dw: usize,
    dh: usize,
    img: &crate::res::Img,
    x: i32,
    y: i32,
    clip: (i32, i32, i32, i32),
) {
    let (cx, cy, cw, ch) = clip;
    let x0 = x.max(cx).max(0);
    let y0 = y.max(cy).max(0);
    let x1 = (x + img.w as i32).min(cx + cw).min(dw as i32);
    let y1 = (y + img.h as i32).min(cy + ch).min(dh as i32);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let iw = img.w as usize;
    for yy in y0..y1 {
        for xx in x0..x1 {
            let si = ((yy - y) as usize * iw + (xx - x) as usize) * 4;
            if img.data[si + 3] == 0 {
                continue;
            }
            let di = (yy as usize * dw + xx as usize) * 4;
            dst[di..di + 4].copy_from_slice(&img.data[si..si + 4]);
        }
    }
}

/// True when an entry's `+0x06` condition is absent (`-1`) or its `B4` program
/// evaluates true (`FUN_00434FE8`).
fn entry_condition_ok(cond: i16, eval: &mut dyn FnMut(u16) -> bool) -> bool {
    cond < 0 || eval(cond as u16)
}

/// Build the display page for a node exactly like `FUN_00434C74`: the first
/// `conditional` (f7) entries are the NPC statement lines, the next
/// `unconditional` (f6) entries are the selectable options. An entry is included
/// only when its condition is absent or evaluates true; a passing statement
/// entry's `B5` action is queued (its `on_open_actions`).
fn build_page(
    quest: &QuestData,
    fonts: &Fonts,
    node: usize,
    logical_w: i32,
    eval: &mut dyn FnMut(u16) -> bool,
) -> Option<(usize, Vec<String>, Vec<DialogOption>, Vec<u16>)> {
    let lay = layout(logical_w);
    let n = quest.node(node)?;
    let entries = quest.entries(node);
    let prompt_n = n.conditional as usize;
    let opt_n = n.unconditional as usize;

    let mut prompt_lines: Vec<String> = Vec::new();
    let mut on_open_actions: Vec<u16> = Vec::new();
    for &e in entries.iter().take(prompt_n) {
        let Some(o) = quest.option(e) else { continue };
        if !entry_condition_ok(o.condition, eval) {
            continue;
        }
        if o.action >= 0 {
            on_open_actions.push(o.action as u16);
        }
        let Some(line) = o.text_line else { continue };
        let t = quest.line_text(line);
        if !t.trim().is_empty() {
            prompt_lines.extend(wrap(fonts, &t, lay.text_width));
        }
    }

    let mut options: Vec<DialogOption> = Vec::new();
    let mut y = lay.opt_pos.1;
    for &e in entries.iter().skip(prompt_n).take(opt_n) {
        let Some(o) = quest.option(e) else { continue };
        if !entry_condition_ok(o.condition, eval) {
            continue;
        }
        let label = o.text_line.map(|l| quest.line_text(l)).unwrap_or_default();
        if label.trim().is_empty() && o.next == LINK_NONE {
            continue;
        }
        let lines = wrap(fonts, &label, lay.text_width);
        let h = (lines.len() as i32 * LINE_H).max(LINE_H);
        options.push(DialogOption {
            lines,
            next: o.next,
            record: e,
            action: o.action,
            row: (lay.opt_pos.0, y, lay.text_width, h),
        });
        y += h;
    }
    Some((node, prompt_lines, options, on_open_actions))
}

/// Resolve a start/branch chain to the first display page (`FUN_00434C74`).
///
/// A branch page (`+0x0A == -2`) selects the first case record whose condition is
/// absent or holds and follows its `+0x04`; `-1`/`-2`, or no matching case,
/// ends the dialogue. Then build the display page.
fn resolve(
    quest: &QuestData,
    fonts: &Fonts,
    start: usize,
    logical_w: i32,
    eval: &mut dyn FnMut(u16) -> bool,
) -> Option<(usize, Vec<String>, Vec<DialogOption>, Vec<u16>)> {
    let mut cur = start;
    for _ in 0..MAX_RESOLVE_HOPS {
        let n = quest.node(cur)?;
        if n.is_branch() {
            let mut next: Option<i16> = None;
            for &c in &quest.entries(cur) {
                let Some(o) = quest.option(c) else { continue };
                if entry_condition_ok(o.condition, eval) {
                    next = Some(o.next);
                    break;
                }
            }
            match next {
                Some(nx) if nx >= 0 => {
                    cur = nx as usize;
                    continue;
                }
                _ => return None,
            }
        }
        return build_page(quest, fonts, cur, logical_w, eval);
    }
    None
}

impl DialogState {
    /// Open the dialogue for `dialogue_id` on `npc_entity`. `eval` evaluates a
    /// `B4` condition-program id (`FUN_00434FE8`). Returns `None` when the id has
    /// no start node or resolves to an end link.
    pub fn start(
        quest: &QuestData,
        fonts: &Fonts,
        npc_entity: usize,
        dialogue_id: u8,
        logical_w: i32,
        eval: &mut dyn FnMut(u16) -> bool,
    ) -> Option<DialogState> {
        let start = quest.dialogue_start(dialogue_id)?;
        let (page, prompt_lines, options, on_open_actions) =
            resolve(quest, fonts, start, logical_w, eval)?;
        Some(DialogState {
            npc_entity,
            page,
            prompt_lines,
            options,
            selected: None,
            on_open_actions,
        })
    }

    /// Select option `index`. Returns `true` when the dialog should stay open,
    /// `false` when it ended (end link, dead end, or no such option). The
    /// caller runs the option's `B5` action (`FUN_00434BB8`) before this call.
    pub fn select(
        &mut self,
        quest: &QuestData,
        fonts: &Fonts,
        index: usize,
        logical_w: i32,
        eval: &mut dyn FnMut(u16) -> bool,
    ) -> bool {
        let Some(next) = self.options.get(index).map(|o| o.next) else {
            return true;
        };
        if next == LINK_END {
            return false;
        }
        // `-1` falls back to the current page; otherwise follow the link.
        let target = if next == LINK_NONE {
            self.page
        } else {
            next as usize
        };
        match resolve(quest, fonts, target, logical_w, eval) {
            Some((page, prompt_lines, options, on_open_actions)) => {
                self.page = page;
                self.prompt_lines = prompt_lines;
                self.options = options;
                self.selected = None;
                self.on_open_actions = on_open_actions;
                true
            }
            None => false,
        }
    }

    /// Option index under a virtual-space `(x, y)`.
    pub fn option_at(&self, x: i32, y: i32) -> Option<usize> {
        self.options.iter().position(|o| contains(o.row, x, y))
    }
}

/// Draw the dialog over the frame (INTR[1] window + INTR[0] band, then the NPC
/// statement and the option list with the original fonts).
pub fn draw(frame: &mut [u8], fw: usize, fh: usize, res: &mut Res, state: &DialogState) {
    let lay = layout(fw as i32);

    let win = res.intr_sprite(WINDOW_SPRITE);
    // Stone background: the original `FUN_00430430` tiles INTR[5] (100x100) over
    // the window rect before drawing the (largely transparent) border. Without it
    // the world shows through the frame (the "no lining" look).
    if let Some(w) = &win {
        let rect = (lay.window_pos.0, lay.window_pos.1, w.w as i32, w.h as i32);
        if let Some(tile) = res.intr_sprite(TILE_SPRITE) {
            for (tx, ty) in tile_origins(rect, TILE) {
                blit_clip(frame, fw, fh, &tile, tx, ty, rect);
            }
        }
        blit(frame, fw, fh, &w.data, w.w, w.h, rect.0, rect.1);
    }
    let band = res.intr_sprite(BAND_SPRITE);
    if let Some(b) = &band {
        blit(
            frame,
            fw,
            fh,
            &b.data,
            b.w,
            b.h,
            lay.band_pos.0,
            lay.band_pos.1,
        );
    }

    let small = res.fonts.small.as_ref();
    let highlight = res.fonts.highlighted.as_ref();

    // NPC statement lines.
    if let Some(font) = small {
        let mut y = lay.text_pos.1;
        for line in &state.prompt_lines {
            font.draw(frame, fw, fh, line, lay.text_pos.0, y, None);
            y += LINE_H;
        }
    }

    // Option list; the selected option uses the highlight font.
    let mut y = lay.opt_pos.1;
    for (i, opt) in state.options.iter().enumerate() {
        let selected = state.selected == Some(i);
        if let Some(font) = small {
            match (selected, highlight) {
                (true, Some(hi)) => {
                    let mut ly = y;
                    for line in &opt.lines {
                        hi.draw(frame, fw, fh, line, lay.opt_pos.0, ly, None);
                        ly += LINE_H;
                    }
                }
                _ => {
                    let rgb = selected.then_some([255, 235, 120]);
                    let mut ly = y;
                    for line in &opt.lines {
                        font.draw(frame, fw, fh, line, lay.opt_pos.0, ly, rgb);
                        ly += LINE_H;
                    }
                }
            }
        }
        y += (opt.lines.len() as i32 * LINE_H).max(LINE_H);
    }
}

#[cfg(test)]
mod tests;
