//! Quest database diagnostics.

use super::super::*;

// --------------------------------------------------------------------------
// Debug: dialogue/quest database (diagnostic tooling; same decoder as runtime)
// --------------------------------------------------------------------------


/// Number of dialogue ids in the loaded `QUEST_R.RES`, or 0 when absent or the
/// structural validation fails. Diagnostic only.
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
            0
        }
    }
}

