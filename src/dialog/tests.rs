//! Unit tests for the [`crate::dialog`] state machine.

use super::*;
use crate::quest::{QuestData, B2_OFF, B3_OFF, B6_OFF, FILE_LEN, NODE_STRIDE};

/// B6 line indices used by the synthetic tests. They are *word offsets* into
/// block B6, so they must be far enough apart that their terminators do not
/// overlap (unlike real data, which packs lines sequentially).
const LINE_A: u16 = 10;
const LINE_B: u16 = 20;

fn blank() -> Vec<u8> {
    vec![0u8; FILE_LEN]
}

fn put_u16(b: &mut [u8], off: usize, v: u16) {
    b[off..off + 2].copy_from_slice(&v.to_le_bytes());
}

fn put_u32(b: &mut [u8], off: usize, v: u32) {
    b[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

/// Write one B3 string at `off` (NUL-terminated).
fn b3_str(b: &mut [u8], off: usize, s: &[u8]) {
    b[B3_OFF + off..B3_OFF + off + s.len()].copy_from_slice(s);
    b[B3_OFF + off + s.len()] = 0;
}

/// Define B6 line `idx` = `[b3_off, 0xFFFF]`.
fn b6_line(b: &mut [u8], idx: u16, b3_off: u16) {
    put_u16(b, B6_OFF + idx as usize * 2, b3_off);
    put_u16(b, B6_OFF + idx as usize * 2 + 2, 0xFFFF);
}

/// Write a display header at `node` with `prompts` conditional + `opts`
/// unconditional entries.
fn header(b: &mut [u8], node: usize, prompts: u16, opts: u16) {
    let o = node * NODE_STRIDE;
    put_u16(b, o, 0xFFFF);
    put_u16(b, o + 2, 0xFFFF);
    put_u16(b, o + 10, 0xFFFF); // action != -2 => display
    put_u16(b, o + 12, opts); // unconditional (f6)
    put_u16(b, o + 14, prompts); // conditional (f7)
}

fn text_entry(b: &mut [u8], node: usize, line: u16, next: i16) {
    let o = node * NODE_STRIDE;
    put_u32(b, o, line as u32);
    put_u16(b, o + 4, next as u16);
    put_u16(b, o + 6, 0xFFFF);
    put_u16(b, o + 10, 0xFFFF);
}

#[test]
fn start_builds_prompt_and_options() {
    let mut b = blank();
    put_u16(&mut b, B2_OFF + 3 * 2, 10);
    header(&mut b, 10, 1, 1);
    text_entry(&mut b, 11, LINE_A, LINK_END);
    text_entry(&mut b, 12, LINE_B, LINK_END);
    b6_line(&mut b, LINE_A, 0);
    b6_line(&mut b, LINE_B, 0);
    b3_str(&mut b, 0, b"Hi");

    let q = QuestData::parse(&b).unwrap();
    let fonts = crate::font::Fonts::empty();
    let d = DialogState::start(&q, &fonts, 277, 3, 800, &mut |_| false).unwrap();
    assert_eq!(d.npc_entity, 277);
    assert_eq!(d.page, 10);
    assert_eq!(d.prompt_lines, vec!["Hi".to_string()]);
    assert_eq!(d.options.len(), 1);
    assert_eq!(d.options[0].label(), "Hi");
    assert_eq!(d.options[0].next, LINK_END);
}

#[test]
fn select_end_link_closes() {
    let mut b = blank();
    put_u16(&mut b, B2_OFF + 3 * 2, 10);
    header(&mut b, 10, 0, 1);
    text_entry(&mut b, 11, LINE_A, LINK_END);
    b6_line(&mut b, LINE_A, 0);
    b3_str(&mut b, 0, b"Hi");
    let q = QuestData::parse(&b).unwrap();
    let fonts = crate::font::Fonts::empty();
    let mut d = DialogState::start(&q, &fonts, 1, 3, 800, &mut |_| false).unwrap();
    assert_eq!(d.options.len(), 1);
    assert!(
        !d.select(&q, &fonts, 0, 800, &mut |_| false),
        "end link must close"
    );
}

#[test]
fn select_follows_link() {
    let mut b = blank();
    put_u16(&mut b, B2_OFF + 3 * 2, 10);
    header(&mut b, 10, 0, 1);
    text_entry(&mut b, 11, LINE_A, 20); // -> node 20
    header(&mut b, 20, 1, 0); // page with one prompt, no options
    text_entry(&mut b, 21, LINE_B, LINK_NONE);
    b6_line(&mut b, LINE_A, 0);
    b6_line(&mut b, LINE_B, 0);
    b3_str(&mut b, 0, b"Hi");
    let q = QuestData::parse(&b).unwrap();
    let fonts = crate::font::Fonts::empty();
    let mut d = DialogState::start(&q, &fonts, 1, 3, 800, &mut |_| false).unwrap();
    assert!(d.select(&q, &fonts, 0, 800, &mut |_| false));
    assert_eq!(d.page, 20);
    assert!(d.options.is_empty());
}

#[test]
fn branch_node_is_followed() {
    let mut b = blank();
    put_u16(&mut b, B2_OFF + 3 * 2, 10);
    // Branch node 10: action -2, one case (the header record itself) with no
    // condition (`-1`) whose `+0x04` next is 11.
    let o = 10 * NODE_STRIDE;
    put_u16(&mut b, o, 0xFFFF);
    put_u16(&mut b, o + 2, 0xFFFF);
    put_u16(&mut b, o + 4, 11);
    put_u16(&mut b, o + 6, 0xFFFF); // first case condition: none
    put_u16(&mut b, o + 10, 0xFFFE); // branch marker
    put_u16(&mut b, o + 14, 1); // one case
    header(&mut b, 11, 1, 0);
    text_entry(&mut b, 12, LINE_A, LINK_NONE);
    b6_line(&mut b, LINE_A, 0);
    b3_str(&mut b, 0, b"Hi");
    let q = QuestData::parse(&b).unwrap();
    let fonts = crate::font::Fonts::empty();
    let d = DialogState::start(&q, &fonts, 1, 3, 800, &mut |_| false).unwrap();
    assert_eq!(d.page, 11);
    assert_eq!(d.prompt_lines, vec!["Hi".to_string()]);
}

#[test]
fn option_hit_testing() {
    let mut b = blank();
    put_u16(&mut b, B2_OFF + 3 * 2, 10);
    header(&mut b, 10, 0, 2);
    text_entry(&mut b, 11, LINE_A, LINK_END);
    text_entry(&mut b, 12, LINE_B, LINK_END);
    b6_line(&mut b, LINE_A, 0);
    b6_line(&mut b, LINE_B, 0);
    b3_str(&mut b, 0, b"Hi");
    let q = QuestData::parse(&b).unwrap();
    let fonts = crate::font::Fonts::empty();
    let d = DialogState::start(&q, &fonts, 1, 3, 800, &mut |_| false).unwrap();
    assert_eq!(d.options.len(), 2);
    let r0 = d.options[0].row;
    assert_eq!(d.option_at(r0.0 + 1, r0.1 + 1), Some(0));
    assert_eq!(d.option_at(-100, -100), None);
}

#[test]
fn tile_origins_cover_window_rect() {
    // 800x600 dialog window: INTR[1] is 651x470 at (144, 0).
    let rect = (144, 0, 651, 470);
    let origins = tile_origins(rect, TILE);
    assert_eq!(origins.len(), 7 * 5);
    assert_eq!(origins[0], (144, 0));
    // Covers the whole rect: every origin is inside, and origins are 100 apart.
    for &(x, y) in &origins {
        assert!(x >= rect.0 && y >= rect.1);
        assert!(x < rect.0 + rect.2 && y < rect.1 + rect.3);
    }
    let max_x = origins.iter().map(|p| p.0).max().unwrap();
    let max_y = origins.iter().map(|p| p.1).max().unwrap();
    assert!(max_x + TILE >= rect.0 + rect.2, "columns cover width");
    assert!(max_y + TILE >= rect.1 + rect.3, "rows cover height");
}

/// End-to-end against the shipped `QUEST_R.RES`, gated on file presence.
#[test]
fn opens_shipped_dialogue() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let data = ["Quest_r.res", "QUEST_R.RES", "QUESTR.RES"]
        .iter()
        .find_map(|n| std::fs::read(dir.join(n)).ok());
    let Some(data) = data else {
        eprintln!("skipping: QUEST_R.RES not present");
        return;
    };
    let q = QuestData::parse(&data).unwrap();
    let fonts = crate::font::Fonts::empty();
    let d = DialogState::start(&q, &fonts, 277, 140, 800, &mut |_| true).expect("dialogue 140 opens");
    let prompt = d.prompt_lines.join(" ");
    assert!(prompt.contains("Князь"), "unexpected prompt: {prompt}");
    assert!(!d.options.is_empty(), "expected at least one option");
}
