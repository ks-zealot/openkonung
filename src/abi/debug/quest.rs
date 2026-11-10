//! Quest database diagnostics.

use super::super::*;

// --------------------------------------------------------------------------
// Debug: dialogue/quest database (diagnostic tooling; same decoder as runtime)
// --------------------------------------------------------------------------

/// Number of dialogue ids in the loaded `QUEST_R.RES`, or 0 when absent or the
/// structural validation fails. Diagnostic only.
///
/// A successful parse reports the fixed `B2` dialogue table length
/// (`DIALOGUE_ENTRIES`), which is what the smoke compares against the shipped
/// file rather than a count of non-empty nodes.
#[no_mangle]
pub extern "C" fn debug_quest_dialogue_count() -> u32 {
    let bytes = loader_mut().quest.clone();
    if bytes.is_empty() {
        return 0;
    }
    match quest::QuestData::parse(&bytes) {
        Ok(_) => quest::DIALOGUE_ENTRIES as u32,
        Err(e) => {
            eprintln!("{e}");
            return 0;
        }
    }
}

/// Diagnostic: activate the first `count` B7 slots with a non-empty B6 journal
/// line (preview only; the real line is written by quest actions).
///
/// Each touched slot gets the `0x80` active bit and the chosen line index in its
/// high half. Returns the number of slots activated (0 when no game or no
/// `QUEST_R.RES` is loaded).
#[no_mangle]
pub extern "C" fn debug_activate_journal_slots(count: u32) -> u32 {
    let Some(g) = runtime_mut().game.as_deref_mut() else {
        return 0;
    };
    let Some(q) = g.world.res.quest.as_ref() else {
        return 0;
    };
    let line = (0..(quest::B6_LEN / 2) as u32)
        .find(|&l| !q.line_text(l as u16).trim().is_empty())
        .unwrap_or(0) as u16;

    let mut n = 0u32;
    for i in 0..g.quest_flags.len().min(300) {
        if n >= count {
            break;
        }
        let low = g.quest_flags[i] & 0xFFFF;
        g.quest_flags[i] = low | ((line as u32) << 16) | 0x80;
        n += 1;
    }
    n
}
