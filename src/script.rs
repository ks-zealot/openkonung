//! Original Konung **quest/dialogue script opcodes** (`op >= 0x112`).
//!
//! The dialogue condition (`B4`) and action (`B5`) programs are lists of `u16`
//! words (`DIALOG_SYSTEM.md`). A word's low 9 bits are the operand and the high
//! bits are flags: [`crate::quest::OP_FLAG_SCRIPT`] (`0x2000`) selects the script
//! opcode dispatcher `FUN_0043159C`; `0x4000` is a condition group separator and
//! `0x8000` negates/sets a `B7` flag.
//!
//! `FUN_0043159C(op)` handles `op < 0x112` with five range/strided dispatch
//! families and, for `op >= 0x112`, calls a function pointer from the table at
//! VA `0x465BD4` indexed by `op` (`CALL [op*4 + 0x465BD4]`). The table is
//! op-indexed, so entries below `0x112` are unused filler; the real table begins
//! at `0x46601C`. Shipped dialogues use `op` `0x112..=0x16E`.
//!
//! Every handler in that range is catalogued below with its VA and whether it is
//! used as a **predicate** (its boolean result is meaningful) or an **action**
//! (side effect; the result is ignored by the action executor). Recovered from
//! `konung.res` via REA (see `DIALOG_SYSTEM.md` §7).

/// First opcode dispatched through the `0x465BD4` pointer table.
pub const OP_BASE: u16 = 0x112;
/// Last opcode with a handler used by the shipped dialogues.
pub const OP_LAST: u16 = 0x16E;
/// The trade / inventory-exchange opcode (`FUN_00431E2C`).
pub const TRADE_OP: u16 = 0x113;

/// How an opcode's return value is used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpKind {
    /// Result is a boolean condition (used by `FUN_00434FE8`).
    Predicate,
    /// Performs a side effect; the result is ignored (`FUN_00434BB8`).
    Action,
    /// Boolean result **and** a state change.
    Both,
}

/// One recovered script opcode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScriptOp {
    pub op: u16,
    /// Handler virtual address in `konung.res`.
    pub handler: u32,
    pub kind: OpKind,
    /// Short recovered effect description.
    pub effect: &'static str,
}

use OpKind::{Action, Both, Predicate};

/// The `0x112..=0x16E` opcode catalogue (index = `op - OP_BASE`).
pub const OPS: &[ScriptOp] = &[
    ScriptOp {
        op: 0x112,
        handler: 0x431DCC,
        kind: Action,
        effect: "mark NPC group visited; OR settlement progress bits 0x15",
    },
    ScriptOp {
        op: 0x113,
        handler: 0x431E2C,
        kind: Action,
        effect: "trade/loot: load player+partner inventories/money, open exchange mode 7",
    },
    ScriptOp {
        op: 0x114,
        handler: 0x4346B0,
        kind: Action,
        effect: "set protagonist +0xD3 = 0x46",
    },
    ScriptOp {
        op: 0x115,
        handler: 0x433F28,
        kind: Predicate,
        effect: "rng()%100 < 50",
    },
    ScriptOp {
        op: 0x116,
        handler: 0x4337A8,
        kind: Predicate,
        effect: "skill/stat check FUN_004313B0(4)",
    },
    ScriptOp {
        op: 0x117,
        handler: 0x432F10,
        kind: Predicate,
        effect: "skill/stat check FUN_004313B0(1)",
    },
    ScriptOp {
        op: 0x118,
        handler: 0x4335EC,
        kind: Predicate,
        effect: "skill/stat check FUN_004313B0(3)",
    },
    ScriptOp {
        op: 0x119,
        handler: 0x4330C4,
        kind: Predicate,
        effect: "skill/stat check FUN_004313B0(2)",
    },
    ScriptOp {
        op: 0x11A,
        handler: 0x434B68,
        kind: Both,
        effect: "if settlement and money>=120: clear flag 0x20, pay 120",
    },
    ScriptOp {
        op: 0x11B,
        handler: 0x433A40,
        kind: Action,
        effect: "add settlement[+0x10] to money, zero it",
    },
    ScriptOp {
        op: 0x11C,
        handler: 0x433824,
        kind: Action,
        effect: "if settlement: set flag 0x20, +100 gold",
    },
    ScriptOp {
        op: 0x11D,
        handler: 0x433868,
        kind: Action,
        effect: "gold -5",
    },
    ScriptOp {
        op: 0x11E,
        handler: 0x433890,
        kind: Action,
        effect: "gold -10",
    },
    ScriptOp {
        op: 0x11F,
        handler: 0x4338B8,
        kind: Action,
        effect: "gold -20",
    },
    ScriptOp {
        op: 0x120,
        handler: 0x4338E0,
        kind: Action,
        effect: "gold -50",
    },
    ScriptOp {
        op: 0x121,
        handler: 0x433908,
        kind: Action,
        effect: "gold -100",
    },
    ScriptOp {
        op: 0x122,
        handler: 0x433930,
        kind: Action,
        effect: "gold -250",
    },
    ScriptOp {
        op: 0x123,
        handler: 0x431820,
        kind: Action,
        effect: "spawn/insert target NPC into world group",
    },
    ScriptOp {
        op: 0x124,
        handler: 0x43453C,
        kind: Both,
        effect: "add quest item (class 8/0x12); true if slot free",
    },
    ScriptOp {
        op: 0x125,
        handler: 0x4346DC,
        kind: Action,
        effect: "gold +5",
    },
    ScriptOp {
        op: 0x126,
        handler: 0x434704,
        kind: Action,
        effect: "gold +10",
    },
    ScriptOp {
        op: 0x127,
        handler: 0x43472C,
        kind: Action,
        effect: "gold +20",
    },
    ScriptOp {
        op: 0x128,
        handler: 0x434754,
        kind: Action,
        effect: "gold +50",
    },
    ScriptOp {
        op: 0x129,
        handler: 0x43477C,
        kind: Action,
        effect: "gold +100",
    },
    ScriptOp {
        op: 0x12A,
        handler: 0x4347A4,
        kind: Action,
        effect: "gold +250",
    },
    ScriptOp {
        op: 0x12B,
        handler: 0x4345E0,
        kind: Action,
        effect: "grant 10 XP",
    },
    ScriptOp {
        op: 0x12C,
        handler: 0x434610,
        kind: Action,
        effect: "grant 20 XP",
    },
    ScriptOp {
        op: 0x12D,
        handler: 0x434640,
        kind: Action,
        effect: "grant 50 XP",
    },
    ScriptOp {
        op: 0x12E,
        handler: 0x43425C,
        kind: Predicate,
        effect: "settlement flag 0x10 set",
    },
    ScriptOp {
        op: 0x12F,
        handler: 0x432D64,
        kind: Predicate,
        effect: "protagonist has dialogue id != 0xFF",
    },
    ScriptOp {
        op: 0x130,
        handler: 0x433760,
        kind: Predicate,
        effect: "settlement word +0x3F2 != 0",
    },
    ScriptOp {
        op: 0x131,
        handler: 0x4330F8,
        kind: Predicate,
        effect: "money > 29",
    },
    ScriptOp {
        op: 0x132,
        handler: 0x43312C,
        kind: Predicate,
        effect: "money >= 40",
    },
    ScriptOp {
        op: 0x133,
        handler: 0x433160,
        kind: Predicate,
        effect: "money >= 50",
    },
    ScriptOp {
        op: 0x134,
        handler: 0x433194,
        kind: Predicate,
        effect: "money >= 100",
    },
    ScriptOp {
        op: 0x135,
        handler: 0x4331C8,
        kind: Predicate,
        effect: "money >= 500",
    },
    ScriptOp {
        op: 0x136,
        handler: 0x4331FC,
        kind: Predicate,
        effect: "money >= 300 (500/halved by settlement flag 0x10)",
    },
    ScriptOp {
        op: 0x137,
        handler: 0x4336A4,
        kind: Predicate,
        effect: "settlement[+0x10] != 0",
    },
    ScriptOp {
        op: 0x138,
        handler: 0x4336E8,
        kind: Predicate,
        effect: "current settlement present",
    },
    ScriptOp {
        op: 0x139,
        handler: 0x433454,
        kind: Predicate,
        effect: "owns item class 9 subtype 1",
    },
    ScriptOp {
        op: 0x13A,
        handler: 0x432EC8,
        kind: Predicate,
        effect: "settlement word +0x3DA != 0",
    },
    ScriptOp {
        op: 0x13B,
        handler: 0x432F44,
        kind: Predicate,
        effect: "settlement[+0x1E]>>24 == 3",
    },
    ScriptOp {
        op: 0x13C,
        handler: 0x433718,
        kind: Predicate,
        effect: "settlement[+0x36]>>24 == 3",
    },
    ScriptOp {
        op: 0x13D,
        handler: 0x4337DC,
        kind: Predicate,
        effect: "settlement[+0x46]>>24 == 3",
    },
    ScriptOp {
        op: 0x13E,
        handler: 0x4334D8,
        kind: Predicate,
        effect: "owns item class 9 subtype 2",
    },
    ScriptOp {
        op: 0x13F,
        handler: 0x4335A4,
        kind: Predicate,
        effect: "settlement word +0x3EA != 0",
    },
    ScriptOp {
        op: 0x140,
        handler: 0x432CE0,
        kind: Predicate,
        effect: "settlement[+0x2E]>>24 == 3",
    },
    ScriptOp {
        op: 0x141,
        handler: 0x43307C,
        kind: Predicate,
        effect: "settlement word +0x3E2 != 0",
    },
    ScriptOp {
        op: 0x142,
        handler: 0x43355C,
        kind: Predicate,
        effect: "settlement[+0x26]>>24 == 3",
    },
    ScriptOp {
        op: 0x143,
        handler: 0x432DA0,
        kind: Predicate,
        effect: "settlement[+0x3E]>>24 == 3",
    },
    ScriptOp {
        op: 0x144,
        handler: 0x433264,
        kind: Predicate,
        effect: "carries item with template flag 0x659B1A bit0",
    },
    ScriptOp {
        op: 0x145,
        handler: 0x432D28,
        kind: Predicate,
        effect: "protagonist[+0xCF]>>24 >= 0",
    },
    ScriptOp {
        op: 0x146,
        handler: 0x432DE8,
        kind: Predicate,
        effect: "settlement group/strength FUN_00431520 >= 5",
    },
    ScriptOp {
        op: 0x147,
        handler: 0x432E48,
        kind: Predicate,
        effect: "settlement owner group slot record nonzero",
    },
    ScriptOp {
        op: 0x148,
        handler: 0x43339C,
        kind: Predicate,
        effect: "any requirement predicate 0x151..0x157 holds",
    },
    ScriptOp {
        op: 0x149,
        handler: 0x433620,
        kind: Predicate,
        effect: "owns item class 9 subtype 0",
    },
    ScriptOp {
        op: 0x14A,
        handler: 0x4341F0,
        kind: Predicate,
        effect: "protagonist[+0x5A]>>16 >= 1600",
    },
    ScriptOp {
        op: 0x14B,
        handler: 0x433EFC,
        kind: Action,
        effect: "recalc protagonist FUN_0041BA08",
    },
    ScriptOp {
        op: 0x14C,
        handler: 0x432954,
        kind: Action,
        effect: "recruit target NPC into a settlement party slot",
    },
    ScriptOp {
        op: 0x14D,
        handler: 0x4340E8,
        kind: Predicate,
        effect: "settlement flag 0x20 set",
    },
    ScriptOp {
        op: 0x14E,
        handler: 0x433B18,
        kind: Action,
        effect: "set target +0x48 dist value, +0x5C=1600, re-render",
    },
    ScriptOp {
        op: 0x14F,
        handler: 0x433BA0,
        kind: Action,
        effect: "group members +0x48 computed, +0x5C=6, FUN_0042C730",
    },
    ScriptOp {
        op: 0x150,
        handler: 0x432804,
        kind: Both,
        effect: "if settlement flag 0x10 and money>=100: activate settlement group",
    },
    ScriptOp {
        op: 0x151,
        handler: 0x432544,
        kind: Predicate,
        effect: "requirement 0x458A06 for state settlement[+0x1D]>>24",
    },
    ScriptOp {
        op: 0x152,
        handler: 0x4326A4,
        kind: Predicate,
        effect: "requirement for state settlement[+0x35]>>24",
    },
    ScriptOp {
        op: 0x153,
        handler: 0x432754,
        kind: Predicate,
        effect: "requirement for state settlement[+0x45]>>24",
    },
    ScriptOp {
        op: 0x154,
        handler: 0x432314,
        kind: Predicate,
        effect: "requirement for state settlement[+0x2D]>>24",
    },
    ScriptOp {
        op: 0x155,
        handler: 0x4325F4,
        kind: Predicate,
        effect: "requirement for state settlement[+0x25]>>24",
    },
    ScriptOp {
        op: 0x156,
        handler: 0x4323C4,
        kind: Predicate,
        effect: "requirement for state settlement[+0x3D]>>24",
    },
    ScriptOp {
        op: 0x157,
        handler: 0x432474,
        kind: Predicate,
        effect: "requirement for group record slot settlement[0]+7",
    },
    ScriptOp {
        op: 0x158,
        handler: 0x434670,
        kind: Both,
        effect: "if owns item class 0xB subtype 0x22: +0xD2=0x13",
    },
    ScriptOp {
        op: 0x159,
        handler: 0x433FD4,
        kind: Both,
        effect: "prepare settlement ally slot 4 (FUN_00431654(4))",
    },
    ScriptOp {
        op: 0x15A,
        handler: 0x433F68,
        kind: Both,
        effect: "prepare settlement ally slot 1",
    },
    ScriptOp {
        op: 0x15B,
        handler: 0x433FB0,
        kind: Both,
        effect: "prepare settlement ally slot 3",
    },
    ScriptOp {
        op: 0x15C,
        handler: 0x433F8C,
        kind: Both,
        effect: "prepare settlement ally slot 2",
    },
    ScriptOp {
        op: 0x15D,
        handler: 0x434168,
        kind: Predicate,
        effect: "party count <= capacity",
    },
    ScriptOp {
        op: 0x15E,
        handler: 0x434228,
        kind: Predicate,
        effect: "target NPC +5 == 0",
    },
    ScriptOp {
        op: 0x15F,
        handler: 0x434130,
        kind: Predicate,
        effect: "dialog NPC[+0x5A]>>16 > 800",
    },
    ScriptOp {
        op: 0x160,
        handler: 0x4342A4,
        kind: Predicate,
        effect: "target NPC[+0x30]>>24 > 3",
    },
    ScriptOp {
        op: 0x161,
        handler: 0x4342DC,
        kind: Predicate,
        effect: "dialog NPC[+4] & 0x38 != 0",
    },
    ScriptOp {
        op: 0x162,
        handler: 0x434364,
        kind: Action,
        effect: "recruit eligible group member into settlement party slot",
    },
    ScriptOp {
        op: 0x163,
        handler: 0x4321C4,
        kind: Action,
        effect: "write settlement[+0x26]=0x458A06[state]",
    },
    ScriptOp {
        op: 0x164,
        handler: 0x43226C,
        kind: Action,
        effect: "write settlement[+0x3E]=table[state]",
    },
    ScriptOp {
        op: 0x165,
        handler: 0x4322C0,
        kind: Action,
        effect: "write settlement[+0x4E]=table[state]",
    },
    ScriptOp {
        op: 0x166,
        handler: 0x432078,
        kind: Predicate,
        effect: "requirement for state settlement[+0x2D]>>24",
    },
    ScriptOp {
        op: 0x167,
        handler: 0x432218,
        kind: Action,
        effect: "write settlement[+0x2E]=table[state]",
    },
    ScriptOp {
        op: 0x168,
        handler: 0x4320CC,
        kind: Action,
        effect: "write settlement[+0x46]=table[state]",
    },
    ScriptOp {
        op: 0x169,
        handler: 0x432120,
        kind: Action,
        effect: "write each settlement group slot +0x1E = table value",
    },
    ScriptOp {
        op: 0x16A,
        handler: 0x434880,
        kind: Both,
        effect: "group-wide equipment/stat float evaluation",
    },
    ScriptOp {
        op: 0x16B,
        handler: 0x433C94,
        kind: Action,
        effect: "remove target NPC from world, compact group",
    },
    ScriptOp {
        op: 0x16C,
        handler: 0x432928,
        kind: Action,
        effect: "set protagonist +0xD2 = 0xFF",
    },
    ScriptOp {
        op: 0x16D,
        handler: 0x434310,
        kind: Action,
        effect: "clear settlement flags 0x15, zero +0x14/+0x10",
    },
    ScriptOp {
        op: 0x16E,
        handler: 0x4341B8,
        kind: Predicate,
        effect: "protagonist[+0x56]>>16 > 60",
    },
];

/// Catalogue entry for `op`, or `None` when it is outside the table.
pub fn script_op(op: u16) -> Option<&'static ScriptOp> {
    let i = op.checked_sub(OP_BASE)? as usize;
    OPS.get(i).filter(|s| s.op == op)
}

// ---------------------------------------------------------------------------
// Program execution (`FUN_00434FE8` conditions / `FUN_00434BB8` actions)
// ---------------------------------------------------------------------------

use crate::quest::{OP_FLAG_GROUP, OP_FLAG_NEGATE, OP_FLAG_SCRIPT, OP_MASK, PROGRAM_END};

/// Evaluate a `B4` condition program exactly like `FUN_00434FE8`.
///
/// Each word is a term: a `B7` flag slot (bit `0x80`) or, with
/// [`OP_FLAG_SCRIPT`] (`0x2000`), a script opcode predicate; [`OP_FLAG_NEGATE`]
/// (`0x8000`) negates the term. A term with [`OP_FLAG_GROUP`] (`0x4000`) is
/// ANDed into the running result (short-circuit); a term without it restarts the
/// result only once it has already failed (OR-alternative). The shipped
/// dialogue/quest data sets `0x4000` on every term, giving a plain AND.
pub fn evaluate_condition(
    words: &[u16],
    mut flag_active: impl FnMut(usize) -> bool,
    mut eval_op: impl FnMut(u16) -> bool,
) -> bool {
    let mut acc = true;
    for &w in words {
        if w == PROGRAM_END {
            break;
        }
        let group = w & OP_FLAG_GROUP != 0;
        // Matches `FUN_00434FE8`: a group term is evaluated only while the
        // running result is still true (AND short-circuit); a non-group term
        // only once it has failed (OR-alternative).
        if group {
            if !acc {
                continue;
            }
        } else if acc {
            continue;
        }
        let raw = if w & OP_FLAG_SCRIPT != 0 {
            eval_op(w & OP_MASK)
        } else {
            flag_active((w & OP_MASK) as usize)
        };
        acc = if w & OP_FLAG_NEGATE != 0 { !raw } else { raw };
    }
    acc
}

/// One decoded `B5` action-program step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionStep {
    /// Set `B7` flag slot `index` active (`0x8000` present, no `0x2000`).
    SetFlag(usize),
    /// Clear `B7` flag slot `index` if active (no `0x2000`/`0x8000`).
    ClearFlag(usize),
    /// Run script opcode `op` (`0x2000` present).
    Op(u16),
}

/// Decode a `B5` action program into ordered steps (`FUN_00434BB8`).
pub fn action_steps(words: &[u16]) -> Vec<ActionStep> {
    let mut out = Vec::new();
    for &w in words {
        if w == PROGRAM_END {
            break;
        }
        if w & OP_FLAG_SCRIPT != 0 {
            out.push(ActionStep::Op(w & OP_MASK));
        } else if w & OP_FLAG_NEGATE != 0 {
            out.push(ActionStep::SetFlag((w & OP_MASK) as usize));
        } else {
            out.push(ActionStep::ClearFlag((w & OP_MASK) as usize));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_contiguous_and_complete() {
        assert_eq!(OPS.len(), (OP_LAST - OP_BASE + 1) as usize);
        for (i, s) in OPS.iter().enumerate() {
            assert_eq!(s.op, OP_BASE + i as u16);
            assert!(
                s.handler >= 0x400000,
                "handler {:#X} out of range",
                s.handler
            );
        }
    }

    #[test]
    fn trade_op_is_0x113() {
        let t = script_op(TRADE_OP).unwrap();
        assert_eq!(t.handler, 0x431E2C);
        assert_eq!(t.kind, OpKind::Action);
        assert!(t.effect.contains("trade"));
    }

    #[test]
    fn lookup_bounds() {
        assert!(script_op(OP_BASE).is_some());
        assert!(script_op(OP_LAST).is_some());
        assert!(script_op(OP_BASE - 1).is_none());
        assert!(script_op(0xFFFF).is_none());
    }

    #[test]
    fn condition_programs_are_short_circuit_and() {
        use crate::quest::{OP_FLAG_GROUP, OP_FLAG_NEGATE, OP_FLAG_SCRIPT};
        // Single group term: result == the opcode result.
        let one = [OP_FLAG_GROUP | OP_FLAG_SCRIPT | 0x15E];
        assert!(evaluate_condition(&one, |_| false, |op| op == 0x15E));
        assert!(!evaluate_condition(&one, |_| false, |op| op == 0x160));

        // Two group terms: AND (a false first term short-circuits).
        let two = [
            OP_FLAG_GROUP | OP_FLAG_SCRIPT | 0x15E,
            OP_FLAG_GROUP | OP_FLAG_SCRIPT | 0x160,
        ];
        assert!(evaluate_condition(
            &two,
            |_| false,
            |op| op == 0x15E || op == 0x160
        ));
        assert!(!evaluate_condition(&two, |_| false, |op| op == 0x15E));

        // `0x8000` negates a `B7` flag term.
        let neg = [OP_FLAG_GROUP | OP_FLAG_NEGATE | 207];
        assert!(evaluate_condition(&neg, |i| i != 207, |_| false));
        assert!(!evaluate_condition(&neg, |i| i == 207, |_| false));
    }

    #[test]
    fn action_steps_decode_flags_and_ops() {
        use crate::quest::OP_FLAG_NEGATE;
        let words = [OP_FLAG_SCRIPT | 0x123, OP_FLAG_NEGATE | 207, 44];
        let steps = action_steps(&words);
        assert_eq!(
            steps,
            vec![
                ActionStep::Op(0x123),
                ActionStep::SetFlag(207),
                ActionStep::ClearFlag(44)
            ]
        );
    }
}
