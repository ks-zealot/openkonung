//! Unit tests for the [`crate::font`] module.

use super::*;

/// Read the shipped `konung.res`, or `None` when it is absent.
fn konung() -> Option<Vec<u8>> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../konung.res");
    std::fs::read(&p).ok()
}

/// Both font headers carry first byte `0x20`, fallback `4` and 209 glyph slots.
#[test]
fn table_landmarks() {
    let Some(d) = konung() else {
        eprintln!("skipping: konung.res not present");
        return;
    };
    let small = FontTable::parse(&d, SMALL_FONT_OFF).expect("small font");
    let large = FontTable::parse(&d, LARGE_FONT_OFF).expect("large font");
    assert_eq!(small.first_char(), 0x20);
    assert_eq!(large.first_char(), 0x20);
    assert_eq!(small.fallback_width(), 4);
    assert_eq!(large.fallback_width(), 4);
    assert_eq!(small.glyph_count(), 0xD1);
    assert_eq!(large.glyph_count(), 0xD1);
}

/// Latin and CP866 Cyrillic glyphs decode at their documented sizes, and the
/// space glyph is null so it advances by the fallback width.
#[test]
fn latin_and_cyrillic_decode_with_expected_sizes() {
    let Some(d) = konung() else { return };
    let small = FontTable::parse(&d, SMALL_FONT_OFF).unwrap();
    let large = FontTable::parse(&d, LARGE_FONT_OFF).unwrap();
    // Typical 'A' sizes documented by the original-code trace.
    let sa = small.glyph('A').unwrap();
    assert_eq!((sa.width, sa.height), (8, 12));
    let la = large.glyph('A').unwrap();
    assert_eq!((la.width, la.height), (11, 16));
    // CP866 Cyrillic must resolve (А = 0x80).
    assert!(small.glyph('\u{0410}').is_some(), "Cyrillic А decodes");
    // Space is a null glyph -> fallback advance.
    assert!(small.glyph(' ').is_none());
    assert_eq!(small.measure(" "), 4);
}

/// The font is proportional and `measure` returns exactly the advance `draw`
/// produces for the same string.
#[test]
fn widths_are_variable_and_measure_matches_draw() {
    let Some(d) = konung() else { return };
    let small = FontTable::parse(&d, SMALL_FONT_OFF).unwrap();
    let iw = small.glyph('i').map(|g| g.width);
    let mw = small.glyph('M').map(|g| g.width);
    if let (Some(i), Some(m)) = (iw, mw) {
        assert_ne!(i, m, "font must be proportional");
    }
    for s in ["1", "25", "100", "12345"] {
        let measured = small.measure(s);
        let mut buf = vec![0u8; 64 * 32 * 4];
        let drawn = small.draw(&mut buf, 64, 32, s, 0, 0, None);
        assert_eq!(measured, drawn, "measure/draw advance mismatch for {s}");
    }
}

/// Transparent scanline skips leave destination pixels untouched while the
/// glyph body is written opaque.
#[test]
fn transparent_skip_does_not_overwrite_destination() {
    let Some(d) = konung() else { return };
    let small = FontTable::parse(&d, SMALL_FONT_OFF).unwrap();
    let mut buf = vec![0xABu8; 64 * 32 * 4];
    small.draw(&mut buf, 64, 32, "1", 0, 0, None);
    // A pixel well to the right of the glyph must be untouched.
    let di = (0 * 64 + 40) * 4;
    assert_eq!(buf[di], 0xAB);
    // At least one glyph pixel must have been written.
    assert!(buf.chunks_exact(4).any(|p| p[3] == 255));
}
