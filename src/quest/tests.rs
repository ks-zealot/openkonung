//! Unit tests for the [`crate::quest`] decoder.

use super::*;

fn blank() -> Vec<u8> {
    vec![0u8; FILE_LEN]
}

fn put_u16(b: &mut [u8], off: usize, v: u16) {
    b[off..off + 2].copy_from_slice(&v.to_le_bytes());
}

fn put_u32(b: &mut [u8], off: usize, v: u32) {
    b[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

#[test]
fn blocks_are_contiguous_and_sum_to_file_len() {
    assert_eq!(B1_OFF, 0);
    assert_eq!(B2_OFF, B1_OFF + B1_LEN);
    assert_eq!(B3_OFF, B2_OFF + B2_LEN);
    assert_eq!(B4_OFF, B3_OFF + B3_LEN);
    assert_eq!(B5_OFF, B4_OFF + B4_LEN);
    assert_eq!(B6_OFF, B5_OFF + B5_LEN);
    assert_eq!(B7_OFF, B6_OFF + B6_LEN);
    assert_eq!(B7_OFF + B7_LEN, FILE_LEN);
    assert_eq!(NODE_COUNT, 8000);
    assert_eq!(DIALOGUE_ENTRIES, 300);
}

#[test]
fn truncated_is_rejected_not_partially_parsed() {
    assert_eq!(
        QuestData::parse(&blank()[..FILE_LEN - 1]).unwrap_err(),
        QuestError::Truncated {
            need: FILE_LEN,
            got: FILE_LEN - 1
        }
    );
}

#[test]
fn synthetic_page_entries_and_text() {
    let mut b = blank();
    // dialogue_id 5 -> node 7
    put_u16(&mut b, B2_OFF + 5 * 2, 7);
    // display header at 7: next=9, marker=-1, 1 conditional + 1 unconditional
    let h = 7 * NODE_STRIDE;
    put_u16(&mut b, h, 0xFFFF);
    put_u16(&mut b, h + 2, 0xFFFF);
    put_u16(&mut b, h + 4, 9);
    put_u16(&mut b, h + 10, 0xFFFF);
    put_u16(&mut b, h + 12, 1);
    put_u16(&mut b, h + 14, 1);
    // entry 8: text line 3, next 10, no condition/action
    let e = 8 * NODE_STRIDE;
    put_u32(&mut b, e, 3);
    put_u16(&mut b, e + 4, 10);
    put_u16(&mut b, e + 6, 0xFFFF);
    put_u16(&mut b, e + 10, 0xFFFF);
    // B6 line 3 = [offset 5, terminator]
    put_u16(&mut b, B6_OFF + 3 * 2, 5);
    put_u16(&mut b, B6_OFF + 3 * 2 + 2, 0xFFFF);
    // B3 at offset 5 = "Hi"
    b[B3_OFF + 5..B3_OFF + 7].copy_from_slice(b"Hi");
    b[B3_OFF + 7] = 0;

    let q = QuestData::parse(&b).unwrap();
    assert_eq!(q.dialogue_start(5), Some(7));
    assert!(!q.node(7).unwrap().is_branch());
    assert_eq!(q.entries(7), vec![8, 9]);
    let opt = q.option(8).unwrap();
    assert_eq!(opt.text_line, Some(3));
    assert_eq!(opt.next, 10);
    assert_eq!(opt.condition, -1);
    assert_eq!(opt.action, -1);
    assert_eq!(q.line_text(3), "Hi");
}

#[test]
fn synthetic_branch_cases_start_at_header() {
    let mut b = blank();
    let h = 20 * NODE_STRIDE;
    put_u16(&mut b, h, 0xFFFF);
    put_u16(&mut b, h + 2, 0xFFFF);
    // marker -2, 2 cases
    put_u16(&mut b, h + 10, 0xFFFE);
    put_u16(&mut b, h + 14, 2);
    let q = QuestData::parse(&b).unwrap();
    let n = q.node(20).unwrap();
    assert!(n.is_branch());
    assert_eq!(q.entries(20), vec![20, 21]);
}

#[test]
fn line_text_inserts_separator_before_words() {
    let mut b = blank();
    // line 0 = [offset 0 ("Hello"), offset 6 ("world"), end]
    put_u16(&mut b, B6_OFF, 0);
    put_u16(&mut b, B6_OFF + 2, 6);
    put_u16(&mut b, B6_OFF + 4, 0xFFFF);
    b[B3_OFF..B3_OFF + 5].copy_from_slice(b"Hello");
    b[B3_OFF + 5] = 0;
    b[B3_OFF + 6..B3_OFF + 11].copy_from_slice(b"world");
    b[B3_OFF + 11] = 0;
    let q = QuestData::parse(&b).unwrap();
    assert_eq!(q.line_text(0), "Hello world");
}

/// End-to-end against the shipped `QUEST_R.RES`, gated on file presence.
#[test]
fn decodes_shipped_quest_file() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let data = ["Quest_r.res", "QUEST_R.RES", "QUESTR.RES"]
        .iter()
        .find_map(|n| std::fs::read(dir.join(n)).ok());
    let Some(data) = data else {
        eprintln!("skipping: QUEST_R.RES not present");
        return;
    };
    let q = QuestData::parse(&data).unwrap();

    // Гилли (`Game.1` entity 277) has dialogue_id 140 -> node 3763.
    let start = q.dialogue_start(140).expect("dialogue 140 present");
    assert_eq!(start, 3763);
    let head = q.node(start).unwrap();
    assert!(head.is_header());
    assert!(head.is_branch());
    assert_eq!(head.next, 3765);
    assert_eq!(head.conditional, 2);

    // Node 3765 is a display page with one conditional + one unconditional
    // entry (3766, 3767).
    let page = q.node(3765).unwrap();
    assert!(page.is_header());
    assert!(!page.is_branch());
    assert_eq!(page.conditional, 1);
    assert_eq!(page.unconditional, 1);
    assert_eq!(q.entries(3765), vec![3766, 3767]);

    let opt = q.option(3766).unwrap();
    assert_eq!(opt.text_line, Some(28556));
    assert_eq!(opt.action, 976);
    let text = q.line_text(28556);
    assert!(text.contains("Князь"), "unexpected text: {text}");

    // Action/condition programs terminate and expose the flag/script flags.
    let act = q.action_program(976);
    assert!(!act.is_empty() && act.iter().all(|&w| w != PROGRAM_END));
    assert!(act.iter().any(|&w| w & OP_FLAG_SCRIPT != 0));
    assert!(act.iter().any(|&w| w & OP_FLAG_NEGATE != 0));
}
