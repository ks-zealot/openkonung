//! Location names from `konung.res`.
//!
//! The original stores a fixed-size CP866 name table at file offset 0x41594:
//! 40-byte records, zero-based, one per map (Konung.N -> record N-1).

/// File offset of the location-name table inside `konung.res`.
pub const NAME_TABLE_OFF: usize = 0x41594;
/// Fixed record size in bytes.
pub const NAME_RECORD: usize = 40;

/// CP866 glyphs for byte values 0x80..0xFF (code points). 0x00..0x7F is ASCII.
const CP866_HIGH: [u16; 128] = [
    1040, 1041, 1042, 1043, 1044, 1045, 1046, 1047, 1048, 1049, 1050, 1051, 1052, 1053, 1054,
    1055, 1056, 1057, 1058, 1059, 1060, 1061, 1062, 1063, 1064, 1065, 1066, 1067, 1068, 1069,
    1070, 1071, 1072, 1073, 1074, 1075, 1076, 1077, 1078, 1079, 1080, 1081, 1082, 1083, 1084,
    1085, 1086, 1087, 9617, 9618, 9619, 9474, 9508, 9569, 9570, 9558, 9557, 9571, 9553, 9559,
    9565, 9564, 9563, 9488, 9492, 9524, 9516, 9500, 9472, 9532, 9566, 9567, 9562, 9556, 9577,
    9574, 9568, 9552, 9580, 9575, 9576, 9572, 9573, 9561, 9560, 9554, 9555, 9579, 9578, 9496,
    9484, 9608, 9604, 9612, 9616, 9600, 1088, 1089, 1090, 1091, 1092, 1093, 1094, 1095, 1096,
    1097, 1098, 1099, 1100, 1101, 1102, 1103, 1025, 1105, 1028, 1108, 1031, 1111, 1038, 1118,
    176, 8729, 183, 8730, 8470, 164, 9632, 160,
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

fn decode_cp866(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| cp866_byte_to_char(b)).collect()
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
