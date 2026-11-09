//! Location names from `konung.res`.
//!
//! The original stores a fixed-size CP866 name table at file offset 0x41594:
//! 40-byte records, zero-based, one per map (Konung.N -> record N-1).

/// File offset of the location-name table inside `konung.res`.
pub const NAME_TABLE_OFF: usize = 0x41594;
/// Fixed record size in bytes.
pub const NAME_RECORD: usize = 40;

/// Character-name table in `konung.res` (DGROUP VA `0x45E71C`): 32-byte CP866
/// records indexed by `Entity+0xD4` (`name_id`). Verified: `124 = Драгомир`,
/// `151 = Михаил`, `10 = Волк`.
pub const CHAR_NAME_TABLE_OFF: usize = 0x4491C;
pub const CHAR_NAME_RECORD: usize = 32;

/// Character-class name table in `konung.res` (DGROUP VA `0x45ADC0`): 16-byte
/// CP866 records indexed by `Entity+0x04 - 1` (`1 Храбрец, 2 Купец,
/// 3 Охотник, 4 Вождь, 5 Воин`).
pub const CLASS_NAME_TABLE_OFF: usize = 0x40FC0;
pub const CLASS_NAME_RECORD: usize = 16;

/// Character nickname/surname table in `konung.res` (DGROUP VA `0x45FC7C`):
/// 32-byte CP866 records indexed by `Entity+0xD5` (`1 Косой, 2 Кривой,
/// 3 Глухой, 4 Слепой, 5 Сирота, ...`). Appended to the name with a space by
/// `FUN_0042BC60`.
pub const NICKNAME_TABLE_OFF: usize = 0x45E7C;
pub const NICKNAME_RECORD: usize = 32;

/// Monster name table in `konung.res` (DGROUP VA `0x45DB3C`): 32-byte CP866
/// records indexed by `Entity+0x04 & 0x7F` (`0x41 Гигантский Муравей`,
/// `0x42 Аспид`, ... `0x50 Ядовитый червь`). Used by `FUN_0042BC60` only for
/// non-human archetypes `<= 0x50`; larger values use the character-name table.
pub const MONSTER_NAME_TABLE_OFF: usize = 0x43D3C;
pub const MONSTER_NAME_RECORD: usize = 32;
pub const MONSTER_NAME_MIN: u8 = 0x41;
pub const MONSTER_NAME_MAX: u8 = 0x50;

/// Settlement waypoint-role names in `konung.res` (DGROUP VA `0x46270C`):
/// 16-byte CP866 records indexed by `FUN_0041182C`'s `1..=5`
/// (`1 староста, 2 знахарь, 3 купец, 4 кузнец, 5 воевода`). The stored records
/// include a leading `", "`; [`profession_name`] strips it.
pub const PROFESSION_TABLE_OFF: usize = 0x4890C;
pub const PROFESSION_RECORD: usize = 16;

/// CP866 glyphs for byte values 0x80..0xFF (code points). 0x00..0x7F is ASCII.
const CP866_HIGH: [u16; 128] = [
    1040, 1041, 1042, 1043, 1044, 1045, 1046, 1047, 1048, 1049, 1050, 1051, 1052, 1053, 1054, 1055,
    1056, 1057, 1058, 1059, 1060, 1061, 1062, 1063, 1064, 1065, 1066, 1067, 1068, 1069, 1070, 1071,
    1072, 1073, 1074, 1075, 1076, 1077, 1078, 1079, 1080, 1081, 1082, 1083, 1084, 1085, 1086, 1087,
    9617, 9618, 9619, 9474, 9508, 9569, 9570, 9558, 9557, 9571, 9553, 9559, 9565, 9564, 9563, 9488,
    9492, 9524, 9516, 9500, 9472, 9532, 9566, 9567, 9562, 9556, 9577, 9574, 9568, 9552, 9580, 9575,
    9576, 9572, 9573, 9561, 9560, 9554, 9555, 9579, 9578, 9496, 9484, 9608, 9604, 9612, 9616, 9600,
    1088, 1089, 1090, 1091, 1092, 1093, 1094, 1095, 1096, 1097, 1098, 1099, 1100, 1101, 1102, 1103,
    1025, 1105, 1028, 1108, 1031, 1111, 1038, 1118, 176, 8729, 183, 8730, 8470, 164, 9632, 160,
];

/// Encode a Unicode scalar to its CP866 byte (ASCII identity; Cyrillic and the
/// CP866 high range via [`CP866_HIGH`]). Returns `None` when the character is
/// not representable in CP866. Used by the original bitmap-font renderer.
pub fn char_to_cp866(ch: char) -> Option<u8> {
    let c = ch as u32;
    if c < 0x80 {
        return Some(c as u8);
    }
    CP866_HIGH
        .iter()
        .position(|&cp| cp as u32 == c)
        .map(|i| 0x80 + i as u8)
}

fn cp866_byte_to_char(b: u8) -> char {
    if b < 0x80 {
        b as char
    } else {
        char::from_u32(CP866_HIGH[(b - 0x80) as usize] as u32).unwrap_or('\u{FFFD}')
    }
}

/// Decode a raw CP866 byte string to Unicode (used for location and item names).
pub fn decode_cp866(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| cp866_byte_to_char(b)).collect()
}

/// Encode a Unicode string to CP866 (inverse of [`decode_cp866`]; characters
/// without a CP866 byte become `?`). Used by the legacy save exporters.
pub fn encode_cp866(text: &str) -> Vec<u8> {
    text.chars()
        .map(|c| {
            let code = c as u32;
            if code < 0x80 {
                code as u8
            } else {
                CP866_HIGH
                    .iter()
                    .position(|&v| v as u32 == code)
                    .map(|i| 0x80 + i as u8)
                    .unwrap_or(b'?')
            }
        })
        .collect()
}

fn decode_record(data: &[u8], off: usize, record: usize) -> Option<String> {
    if off + record > data.len() {
        return None;
    }
    let rec = &data[off..off + record];
    let end = rec.iter().position(|&b| b == 0).unwrap_or(record);
    Some(decode_cp866(&rec[..end]).trim_end().to_string())
}

/// Original character name by `Entity+0xD4` (`name_id`), from the CP866 table
/// at [`CHAR_NAME_TABLE_OFF`]. Returns `None` when out of bounds.
pub fn character_name(data: &[u8], name_id: u8) -> Option<String> {
    let s = decode_record(
        data,
        CHAR_NAME_TABLE_OFF + name_id as usize * CHAR_NAME_RECORD,
        CHAR_NAME_RECORD,
    )?;
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Original class name by `Entity+0x04` (`entity_class`), from the CP866 table
/// at [`CLASS_NAME_TABLE_OFF`]. Returns `None` for id `0` or out of bounds.
pub fn class_name(data: &[u8], entity_class: u8) -> Option<String> {
    if entity_class == 0 {
        return None;
    }
    let s = decode_record(
        data,
        CLASS_NAME_TABLE_OFF + (entity_class as usize - 1) * CLASS_NAME_RECORD,
        CLASS_NAME_RECORD,
    )?;
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Original character nickname/surname by `Entity+0xD5` (`nickname_id`), from
/// the CP866 table at [`NICKNAME_TABLE_OFF`]. Returns `None` for id `0` /
/// out-of-bounds and empty records.
pub fn nickname(data: &[u8], nickname_id: u8) -> Option<String> {
    if nickname_id == 0 {
        return None;
    }
    decode_record(
        data,
        NICKNAME_TABLE_OFF + nickname_id as usize * NICKNAME_RECORD,
        NICKNAME_RECORD,
    )
}

/// Original monster name by `Entity+0x04 & 0x7F`, from the CP866 table at
/// [`MONSTER_NAME_TABLE_OFF`]. Only `0x41..=0x50` are valid names; the table
/// entries below `0x41` are unrelated UI strings.
pub fn monster_name(data: &[u8], archetype_low: u8) -> Option<String> {
    if !(MONSTER_NAME_MIN..=MONSTER_NAME_MAX).contains(&archetype_low) {
        return None;
    }
    decode_record(
        data,
        MONSTER_NAME_TABLE_OFF + archetype_low as usize * MONSTER_NAME_RECORD,
        MONSTER_NAME_RECORD,
    )
}

/// Settlement role name by `FUN_0041182C`'s waypoint slot `1..=5`
/// (`Settlement::role_of`), from the CP866 table at
/// [`PROFESSION_TABLE_OFF`]. The stored record's leading `", "` is stripped.
pub fn profession_name(data: &[u8], role: u8) -> Option<String> {
    if !(1..=5).contains(&role) {
        return None;
    }
    let s = decode_record(
        data,
        PROFESSION_TABLE_OFF + role as usize * PROFESSION_RECORD,
        PROFESSION_RECORD,
    )?;
    let s = s.trim_start_matches(", ").trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Decode the full location-name table. Records are cut at the first NUL and
/// trimmed of trailing whitespace. Parsing stops at the first empty record or
/// at the first record containing a control byte, which marks the end of the
/// name table in the original file.
pub fn parse_location_names(data: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut o = NAME_TABLE_OFF;
    while o + NAME_RECORD <= data.len() {
        let rec = &data[o..o + NAME_RECORD];
        let end = rec.iter().position(|&b| b == 0).unwrap_or(NAME_RECORD);
        let text = &rec[..end];
        // Empty record or a control byte means we have left the name table.
        if text.is_empty() || text.iter().any(|&b| b < 0x20) {
            break;
        }
        out.push(decode_cp866(text).trim_end().to_string());
        o += NAME_RECORD;
    }
    out
}

#[cfg(test)]
mod tests;
