//! Original Konung **dialogue / quest data** decoder (`QUEST_R.RES`).
//!
//! The original opens dialogue by clicking an NPC whose `Entity+0xD0`
//! (`dialogue_id`) is not `0xFF` (gameplay driver `FUN_0042260c` -> dialog entry
//! `FUN_00435290`). The dialogue, condition and action programs all live in
//! `QUEST_R.RES`, loaded once into fixed buffers by `FUN_0043a870` (the same
//! routine that loads `Game.N`); see `DIALOG_SYSTEM.md` for the full trace.
//!
//! The 337 800-byte file is seven fixed blocks, read verbatim into fixed
//! addresses (original file offset -> buffer VA -> length):
//!
//! ```text
//! block  file off   length   VA         role (evidence)
//! B1     0          128000   0x5C2D90   node/option records, stride 0x10
//! B2     128000     600      0x5E2190   u16[300]: dialogue_id -> first node
//! B3     128600     64000    0x5E23E8   CP866 text blob (NUL terminated)
//! B4     192600     12000    0x5F1DE8   u16[] condition programs (0xFFFF ended)
//! B5     204600     12000    0x5F4CC8   u16[] action programs   (0xFFFF ended)
//! B6     216600     120000   0x5F7BA8   u16[] per-line lists of B3 offsets
//! B7     336600     1200     0x615068   u32[300] quest/global flag slots
//! ```
//!
//! `B1` records
//! ------------
//! A record is 16 bytes. The original uses two shapes:
//!
//! * **header / page** (`+0x00` and `+0x02` are `0xFFFF`): `+0x04` is the
//!   next page (or the branch target), `+0x0A` is `0xFFFF` for a display page
//!   and `0xFFFE` for a branch/switcher page, `+0x0C`(low `u16`) is the number
//!   of **unconditional** entry records and `+0x0E`(high `u16`) the number of
//!   **conditional** entry records. `FUN_00434c74` reads the entry count as
//!   `(u32 @ +0x0C) >> 16` and the first conditional pass, then reads the low
//!   word for the unconditional pass, so a display page's entries are the
//!   `conditional + unconditional` records starting at `header+1`. A branch
//!   page's cases are the `conditional` records starting at the header itself.
//! * **entry / option**: `+0x00` (u32) is a **`B6` line index** for the text
//!   (`0xFFFFFFFF` = none), `+0x04` is the next node (`0xFFFF` = fall back to
//!   the page, `0xFFFE` = end dialogue), `+0x06` is a `B4` condition program id
//!   (`0xFFFF` = none), `+0x0A` is a `B5` action program id (`0xFFFF` = none).
//!   The remaining entry words (`+0x02`, `+0x08`, `+0x0C`, `+0x0E`) are always
//!   `0xFFFF` in the shipped file (unused).
//!
//! `B7` flag slots
//! ---------------
//! Block `B7` is `300` four-byte slots. Slot `i` is `{u8 flags, u8 ?, u16 @+2}`
//! where bit `0x80` of `flags` is the "active/set" bit toggled by
//! `FUN_00434BB8`/read by `FUN_00434FE8`, and the `u16` at `+0x02` is a `B6`
//! journal text line (the quest-log renderer `FUN_0042CBD0` case 2 prints a slot
//! when it is active and its line != `0xFFFF`). Slots are initialised from the
//! file and mutated at runtime; see [`QuestData::flag_active`]/[`QuestData::journal_line`].
//!
//! The text field was verified against the shipped file: `B6[28556]` composes
//! to the Черный Бор elder greeting "Приветствую тебя, о Великий Князь! ...".
//!
//! Programs (`B4`/`B5`)
//! -------------------
//! A program is a list of `u16` words ended by `0xFFFF`. Each word's low 9 bits
//! are the operand and the high bits are flags: `0x2000` = run original script
//! opcode `FUN_0043159c(operand)`, `0x4000` = condition group separator (used by
//! the `FUN_00434fe8` condition evaluator), `0x8000` = set/negate the `B7`
//! flag slot `operand`. The opcode space is shared between conditions and
//! actions.
//!
//! The opcode catalogue (handler VAs, predicate/action classification) lives in
//! [`crate::script`]; this module only decodes the containers, so a program here
//! is a raw `Vec<u16>` of words. Execution is `crate::script::evaluate_condition`
//! (B4) and `crate::script::action_steps` (B5), driven from `app/dialogue_exec.rs`
//! and `app/dialogue_ops.rs`.

use crate::names::decode_cp866;

/// Required file size (sum of the seven blocks).
pub const FILE_LEN: usize = 337_800;

/// File offset of block `B1`, the node/option records (stride [`NODE_STRIDE`]).
pub const B1_OFF: usize = 0;
/// Length of block `B1`: `128000` bytes = 8000 records of [`NODE_STRIDE`].
pub const B1_LEN: usize = 128_000;
/// File offset of block `B2`, `u16[300]` mapping `dialogue_id` to its first
/// `B1` node (original buffer VA `0x5E2190`).
pub const B2_OFF: usize = 128_000;
/// Length of block `B2`: `300` `u16` entries ([`DIALOGUE_ENTRIES`]).
pub const B2_LEN: usize = 600;
/// File offset of block `B3`, the CP866 text blob of NUL-terminated fragments
/// (original buffer VA `0x5E23E8`).
pub const B3_OFF: usize = 128_600;
/// Length of block `B3` in bytes.
pub const B3_LEN: usize = 64_000;
/// File offset of block `B4`, the `u16[]` condition programs ended by
/// [`PROGRAM_END`] (original buffer VA `0x5F1DE8`).
pub const B4_OFF: usize = 192_600;
/// Length of block `B4` in bytes.
pub const B4_LEN: usize = 12_000;
/// File offset of block `B5`, the `u16[]` action programs ended by
/// [`PROGRAM_END`] (original buffer VA `0x5F4CC8`).
pub const B5_OFF: usize = 204_600;
/// Length of block `B5` in bytes.
pub const B5_LEN: usize = 12_000;
/// File offset of block `B6`, the `u16[]` per-line lists of `B3` byte offsets
/// (original buffer VA `0x5F7BA8`).
pub const B6_OFF: usize = 216_600;
/// Length of block `B6` in bytes.
pub const B6_LEN: usize = 120_000;
/// File offset of block `B7`, the `u32` quest/global flag slots (original
/// buffer VA `0x615068`, [`FLAG_SLOTS`] records).
pub const B7_OFF: usize = 336_600;
/// Length of block `B7` in bytes: `300` four-byte slots.
pub const B7_LEN: usize = 1_200;

/// Stride of a `B1` record.
pub const NODE_STRIDE: usize = 0x10;
/// Number of `B1` records (`B1_LEN / NODE_STRIDE`).
pub const NODE_COUNT: usize = B1_LEN / NODE_STRIDE;
/// `B2` dialogue-id table length (`dialogue_id -> node`).
pub const DIALOGUE_ENTRIES: usize = B2_LEN / 2;

/// Number of `B7` flag slots (`4`-byte records).
pub const FLAG_SLOTS: usize = B7_LEN / 4;

/// Terminator of a `B4`/`B5` program.
pub const PROGRAM_END: u16 = 0xFFFF;
/// Low operand mask of a program word.
pub const OP_MASK: u16 = 0x01FF;
/// Program word flag: operand is a script opcode (`FUN_0043159c`).
pub const OP_FLAG_SCRIPT: u16 = 0x2000;
/// Program word flag: condition group separator (`FUN_00434fe8`).
pub const OP_FLAG_GROUP: u16 = 0x4000;
/// Program word flag: set/negate the `B7` flag slot.
pub const OP_FLAG_NEGATE: u16 = 0x8000;

/// End-of-dialogue link value (option/header `+0x04`).
pub const LINK_END: i16 = -2;
/// "No link" value (option/header `+0x04`).
pub const LINK_NONE: i16 = -1;

/// The file's unused/zero marker for a `u16` field (`0xFFFF`). A branch page
/// stores it in the unused entry-count word; `B1` entry `+0x02/+0x08/+0x0C/+0x0E`
/// are always this value.
pub const UNUSED: u16 = 0xFFFF;

/// Map the file's `0xFFFF` "unused" word to `0` so entry counts can be added.
///
/// A branch page stores `0xFFFF` in the unused `+0x0C` word; treating it as
/// `65535` would make `entries` walk far past the record.
#[inline]
fn normalize_count(v: u16) -> u16 {
    if v == UNUSED {
        0
    } else {
        v
    }
}

/// Failure to own a valid `QUEST_R.RES` image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuestError {
    /// The buffer is smaller than [`FILE_LEN`]; never partially parsed.
    Truncated {
        /// Bytes required ([`FILE_LEN`]).
        need: usize,
        /// Bytes the caller supplied.
        got: usize,
    },
}

impl core::fmt::Display for QuestError {
    /// Report the required and supplied sizes; the loader aborts before use.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            QuestError::Truncated { need, got } => {
                write!(f, "QUEST_R.RES truncated: need {need} bytes, got {got}")
            }
        }
    }
}

/// One `B1` record (page header or option), raw fields preserved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Node {
    /// Record index (`B1` slot).
    pub index: usize,
    /// `+0x00` u16 (header marker `0xFFFF`).
    pub f0: u16,
    /// `+0x02` u16 (header marker `0xFFFF`).
    pub f1: u16,
    /// `+0x04` i16 — next page / branch target (`-1` fall back, `-2` end).
    pub next: i16,
    /// `+0x06` i16.
    pub f3: i16,
    /// `+0x08` u16.
    pub f4: u16,
    /// `+0x0A` i16 — `-2` on a branch page, `-1` on a display page, otherwise
    /// the entry/option action program id.
    pub action: i16,
    /// `+0x0C` u16 — unconditional entry count.
    pub unconditional: u16,
    /// `+0x0E` u16 — conditional entry count (and branch case count).
    pub conditional: u16,
}

impl Node {
    /// True for the `+0x00/+0x02 == 0xFFFF` page-header shape.
    pub fn is_header(&self) -> bool {
        self.f0 == 0xFFFF && self.f1 == 0xFFFF
    }

    /// True for a `+0x0A == 0xFFFE` branch/switcher page.
    pub fn is_branch(&self) -> bool {
        self.action == LINK_END
    }
}

/// One `B1` entry/option record (a dialogue line or a selectable option).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    /// Record index (`B1` slot).
    pub index: usize,
    /// `+0x00` u32 raw text-line field (`0xFFFFFFFF` = none).
    pub text_raw: u32,
    /// `+0x00` low word — a `B6` line index, or `None` for `0xFFFFFFFF`.
    pub text_line: Option<u16>,
    /// `+0x04` i16 — next node (`-1` fall back to the page, `-2` end dialogue).
    pub next: i16,
    /// `+0x06` i16 — `B4` condition program id (`-1` = unconditional).
    pub condition: i16,
    /// `+0x08` u16 — usually `0xFFFF`; role unresolved.
    pub f4: u16,
    /// `+0x0A` i16 — `B5` action program id (`-1` = none).
    pub action: i16,
    /// `+0x0C` u16 — role unresolved.
    pub f6: u16,
    /// `+0x0E` u16 — role unresolved.
    pub f7: u16,
}

/// Owned `QUEST_R.RES` image with structural accessors.
#[derive(Clone, Debug)]
pub struct QuestData {
    bytes: Vec<u8>,
}

impl QuestData {
    /// Own a copy of a complete `QUEST_R.RES`. Rejects truncated input rather
    /// than exposing a partially valid database.
    pub fn parse(bytes: &[u8]) -> Result<QuestData, QuestError> {
        if bytes.len() < FILE_LEN {
            return Err(QuestError::Truncated {
                need: FILE_LEN,
                got: bytes.len(),
            });
        }
        Ok(QuestData {
            bytes: bytes.to_vec(),
        })
    }

    /// Little-endian `u16` at absolute file offset `off` (a `B1`/`B2`/`B7` field).
    fn u16(&self, off: usize) -> u16 {
        crate::res::u16(&self.bytes, off) as u16
    }

    /// Little-endian `i16` at absolute file offset `off`; link and program-id
    /// fields are signed (`-1` = none, `-2` = end).
    fn i16(&self, off: usize) -> i16 {
        crate::res::i16(&self.bytes, off) as i16
    }

    /// Little-endian `u32` at absolute file offset `off`; only the `B1` entry
    /// `+0x00` text word and the `B7` flag slots are 32-bit.
    fn u32(&self, off: usize) -> u32 {
        let b = &self.bytes[off..off + 4];
        u32::from_le_bytes([b[0], b[1], b[2], b[3]])
    }

    /// `B2`: the first `B1` record for a `dialogue_id`. Returns `None` when the
    /// id maps to an out-of-range node (never a panic).
    pub fn dialogue_start(&self, dialogue_id: u8) -> Option<usize> {
        let idx = dialogue_id as usize;
        if idx >= DIALOGUE_ENTRIES {
            return None;
        }
        let node = self.u16(B2_OFF + idx * 2) as usize;
        (node < NODE_COUNT).then_some(node)
    }

    /// `B1` record by index.
    pub fn node(&self, index: usize) -> Option<Node> {
        if index >= NODE_COUNT {
            return None;
        }
        let o = index * NODE_STRIDE;
        Some(Node {
            index,
            f0: self.u16(o),
            f1: self.u16(o + 2),
            next: self.i16(o + 4),
            f3: self.i16(o + 6),
            f4: self.u16(o + 8),
            action: self.i16(o + 10),
            // `0xFFFF` counts are the file's "unused"/zero convention (e.g. a
            // branch page stores `0xFFFF` in the unused `+0x0C` word); normalise
            // them to 0 so entry counts never overflow.
            unconditional: normalize_count(self.u16(o + 12)),
            conditional: normalize_count(self.u16(o + 14)),
        })
    }

    /// The `B1` entry records belonging to a page/branch. For a display page the
    /// entries follow the header (`header+1`); for a branch page the cases start
    /// at the header itself. Counts come from the header's `+0x0C/+0x0E` words.
    pub fn entries(&self, index: usize) -> Vec<usize> {
        let Some(n) = self.node(index) else {
            return Vec::new();
        };
        if n.is_branch() {
            let count = n.conditional as usize;
            (index..index.saturating_add(count))
                .take_while(|&i| i < NODE_COUNT)
                .collect()
        } else if n.is_header() {
            let count = n.conditional as usize + n.unconditional as usize;
            (index + 1..index + 1 + count)
                .take_while(|&i| i < NODE_COUNT)
                .collect()
        } else {
            Vec::new()
        }
    }

    /// A `B1` entry/option record by index.
    pub fn option(&self, index: usize) -> Option<Entry> {
        let o = index.checked_mul(NODE_STRIDE)?;
        if o + NODE_STRIDE > B1_LEN {
            return None;
        }
        let text_raw = self.u32(o);
        Some(Entry {
            index,
            text_raw,
            text_line: (text_raw != 0xFFFF_FFFF).then_some(text_raw as u16),
            next: self.i16(o + 4),
            condition: self.i16(o + 6),
            f4: self.u16(o + 8),
            action: self.i16(o + 10),
            f6: self.u16(o + 12),
            f7: self.u16(o + 14),
        })
    }

    /// Raw bytes of a `B3` entry at `offset` up to its NUL.
    pub fn raw_b3(&self, offset: usize) -> &[u8] {
        if offset >= B3_LEN {
            return &[];
        }
        let start = B3_OFF + offset;
        let end = B3_OFF + B3_LEN;
        let slice = &self.bytes[start..end];
        let len = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
        &slice[..len]
    }

    /// Compose a dialogue line from its `B6` list, exactly like the original
    /// text builder `FUN_00435188`: each `B6` word is a `B3` offset, terminated
    /// by `0xFFFF`; a single space is inserted before a following fragment whose
    /// first byte is a letter/`-`/digit (the separator string at `0x450618`).
    pub fn line_text(&self, line: u16) -> String {
        let mut out = String::new();
        let mut o = B6_OFF + line as usize * 2;
        let end = B6_OFF + B6_LEN;
        let mut guard = 0usize;
        while o + 2 <= end && guard < 4096 {
            guard += 1;
            let word = self.u16(o);
            if word == PROGRAM_END {
                break;
            }
            let frag = self.raw_b3(word as usize);
            if !out.is_empty() {
                if let Some(&first) = frag.first() {
                    let letter = first > 0x40;
                    let dash = first == 0x2D && frag.get(1).copied().unwrap_or(0) == 0;
                    let digit = first > 0x2F && first < 0x3A;
                    if letter || dash || digit {
                        out.push(' ');
                    }
                }
            }
            out.push_str(&decode_cp866(frag));
            o += 2;
        }
        out
    }

    /// Read a `B4`/`B5` program (words up to `0xFFFF`). `base` is the block
    /// start; `id` indexes it as a `u16` table. Returns an empty vector for an
    /// out-of-range id.
    fn program(&self, base: usize, len: usize, id: u16) -> Vec<u16> {
        let mut out = Vec::new();
        let start = base + id as usize * 2;
        if start + 2 > base + len {
            return out;
        }
        let end = base + len;
        let mut o = start;
        while o + 2 <= end {
            let w = self.u16(o);
            if w == PROGRAM_END {
                break;
            }
            out.push(w);
            o += 2;
            if out.len() > 4096 {
                break;
            }
        }
        out
    }

    /// Every distinct script opcode (`0x2000` words) used by the loaded
    /// `B4`/`B5` blocks, sorted. Diagnostic/census helper.
    pub fn used_script_ops(&self) -> Vec<u16> {
        let mut ops = std::collections::BTreeSet::new();
        for (base, len) in [(B4_OFF, B4_LEN), (B5_OFF, B5_LEN)] {
            let mut o = base;
            let end = base + len;
            while o + 2 <= end {
                let w = self.u16(o);
                if w != PROGRAM_END && w & OP_FLAG_SCRIPT != 0 {
                    ops.insert(w & OP_MASK);
                }
                o += 2;
            }
        }
        ops.into_iter().collect()
    }

    /// `B4` condition program by id.
    pub fn condition_program(&self, id: u16) -> Vec<u16> {
        self.program(B4_OFF, B4_LEN, id)
    }

    /// `B5` action program by id.
    pub fn action_program(&self, id: u16) -> Vec<u16> {
        self.program(B5_OFF, B5_LEN, id)
    }

    /// `B7` flag slot (first byte of the 4-byte record at `index`).
    pub fn flag_byte(&self, index: usize) -> Option<u8> {
        let o = B7_OFF + index * 4;
        (o < B7_OFF + B7_LEN).then(|| self.bytes[o])
    }

    /// `B7` slot active bit (`0x80` of the first byte).
    pub fn flag_active(&self, index: usize) -> bool {
        self.flag_byte(index)
            .map(|b| b & 0x80 != 0)
            .unwrap_or(false)
    }

    /// `B7` slot journal text line (`u16 @ +0x02`, a `B6` line index). `None`
    /// when the slot is out of range or its line field is the unused marker.
    pub fn journal_line(&self, index: usize) -> Option<u16> {
        let o = B7_OFF + index * 4;
        if o + 4 > B7_OFF + B7_LEN {
            return None;
        }
        let w = self.u16(o + 2);
        (w != UNUSED).then_some(w)
    }

    /// A whole `B7` slot (`u32` at VA `0x615068`). The original copies the full
    /// 1200-byte block into its runtime flag array and saves it verbatim; the
    /// low byte carries the `0x80` active bit, `+0x02` the journal line.
    pub fn flag_word(&self, index: usize) -> Option<u32> {
        let o = B7_OFF + index * 4;
        if o + 4 > B7_OFF + B7_LEN {
            return None;
        }
        Some(self.u32(o))
    }

    /// The initial runtime `B7` flag words (300 x u32). The original copies
    /// these into `0x615068` and toggles bit `0x80` at runtime
    /// (`FUN_00434BB8` / read by `FUN_00434FE8`). Slots whose file byte already
    /// has `0x80` set start "active".
    pub fn initial_flags(&self) -> Vec<u32> {
        (0..FLAG_SLOTS).map(|i| self.u32(B7_OFF + i * 4)).collect()
    }
}

#[cfg(test)]
mod tests;
