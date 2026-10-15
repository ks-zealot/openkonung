//! Unit tests for the [`crate::menu_res`] module.

use super::*;

/// Build a single record. `rows` is a list of already-encoded row bytes
/// (each must include its own EOL token).
fn record(w: u16, h: u16, rows: &[Vec<u8>]) -> Vec<u8> {
    assert_eq!(rows.len(), h as usize);
    let mut out = Vec::new();
    out.extend_from_slice(&MAGIC);
    let sum: usize = rows.iter().map(|r| r.len()).sum();
    let payload = 4 + rows.len() * 2 + sum;
    out.extend_from_slice(&(payload as u32).to_le_bytes());
    out.extend_from_slice(&w.to_le_bytes());
    out.extend_from_slice(&h.to_le_bytes());
    for r in rows {
        out.extend_from_slice(&(r.len() as u16).to_le_bytes());
    }
    for r in rows {
        out.extend_from_slice(r);
    }
    out
}

#[test]
fn header_and_single_pixel() {
    // 1x1 opaque red (RGB555 0x7C00).
    let data = record(1, 1, &[vec![0x01, 0x00, 0x7C, 0x00]]);
    let r = MenuRes::parse(&data).unwrap();
    assert_eq!(r.images.len(), 1);
    let im = &r.images[0];
    assert_eq!((im.width, im.height), (1, 1));
    assert_eq!(&im.rgba, &[255, 0, 0, 255]);
}

#[test]
fn transparent_run_advances_without_pixels() {
    // 4x1: two transparent then two literal.
    let row = vec![0x82, 0x02, 0x00, 0x7C, 0xE0, 0x03, 0x00];
    let data = record(4, 1, &[row]);
    let im = &MenuRes::parse(&data).unwrap().images[0];
    assert_eq!(&im.rgba[0..4], &[0, 0, 0, 0]);
    assert_eq!(&im.rgba[4..8], &[0, 0, 0, 0]);
    assert_eq!(&im.rgba[8..12], &[255, 0, 0, 255]);
    // 0x03E0 = green
    assert_eq!(&im.rgba[12..16], &[0, 255, 0, 255]);
}

#[test]
fn eol_ends_row() {
    // 2x2, each row one literal then EOL.
    let data = record(
        2,
        2,
        &[
            vec![0x01, 0x00, 0x7C, 0x00],
            vec![0x81, 0x01, 0x00, 0x7C, 0x00],
        ],
    );
    let im = &MenuRes::parse(&data).unwrap().images[0];
    assert_eq!(&im.rgba[0..4], &[255, 0, 0, 255]);
    // second row starts with one transparent, then red.
    let row2 = (1 * 2) * 4;
    assert_eq!(&im.rgba[row2..row2 + 4], &[0, 0, 0, 0]);
    assert_eq!(&im.rgba[row2 + 4..row2 + 8], &[255, 0, 0, 255]);
}

#[test]
fn multiple_records_reach_eof() {
    let a = record(1, 1, &[vec![0x00]]);
    let b = record(2, 1, &[vec![0x01, 0x00, 0x7C, 0x00]]);
    let mut data = a;
    data.extend_from_slice(&b);
    let r = MenuRes::parse(&data).unwrap();
    assert_eq!(r.images.len(), 2);
    assert_eq!((r.images[0].width, r.images[1].width), (1, 2));
}

#[test]
fn rejects_literal_run_beyond_width() {
    // width 1, literal run of 2.
    let data = record(1, 1, &[vec![0x02, 0x00, 0x7C, 0x00, 0x00, 0x7C, 0x00]]);
    let e = MenuRes::parse(&data).unwrap_err();
    assert!(e.message.contains("exceeds image width"), "{e}");
}

#[test]
fn rejects_transparent_run_beyond_width() {
    let data = record(1, 1, &[vec![0x82, 0x00]]);
    let e = MenuRes::parse(&data).unwrap_err();
    assert!(e.message.contains("transparent run"), "{e}");
}

#[test]
fn rejects_missing_rgb555_bytes() {
    // token claims 2 pixels but only one is present.
    let data = record(2, 1, &[vec![0x02, 0x00, 0x7C, 0x00]]);
    let e = MenuRes::parse(&data).unwrap_err();
    assert!(e.message.contains("RGB555 bytes"), "{e}");
}

#[test]
fn rejects_missing_eol() {
    // One white pixel (0x7FFF -> bytes FF 7F, no 0x00) and no EOL token.
    let e = MenuRes::parse(&record(1, 1, &[vec![0x01, 0xFF, 0x7F]])).unwrap_err();
    assert!(e.message.contains("end-of-row"), "{e}");
}

#[test]
fn rejects_bad_magic() {
    let mut data = record(1, 1, &[vec![0x00]]);
    data[3] = 0x09;
    let e = MenuRes::parse(&data).unwrap_err();
    assert!(e.message.contains("bad magic"), "{e}");
}

#[test]
fn rejects_truncated_payload() {
    let mut data = record(4, 1, &[vec![0x81, 0x01, 0x00, 0x7C, 0x00]]);
    data.truncate(data.len() - 2);
    let e = MenuRes::parse(&data).unwrap_err();
    assert!(e.message.contains("payload") || e.message.contains("row"), "{e}");
}

fn install_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// Real-resource regression: only runs when the original assets are present.
#[test]
fn menu640_real_resource() {
    let path = install_dir().join("MENU640R.RES");
    let Ok(bytes) = std::fs::read(&path) else {
        eprintln!("skipping: {} not present", path.display());
        return;
    };
    let res = MenuRes::parse_named("MENU640R.RES", &bytes).expect("MENU640R.RES parse");
    assert_eq!(res.images.len(), 15, "MENU640R.RES image count");
    assert_eq!((res.images[0].width, res.images[0].height), (640, 480));
    assert_eq!((res.images[2].width, res.images[2].height), (13, 13));
    // First full row is exactly 1287 bytes (640*2 + 6 control + 1 EOL).
    assert_eq!(res.images[0].rgba.len(), 640 * 480 * 4);
}

#[test]
fn newh640_real_resource() {
    let path = install_dir().join("NEWH640R.RES");
    let Ok(bytes) = std::fs::read(&path) else {
        eprintln!("skipping: {} not present", path.display());
        return;
    };
    let res = MenuRes::parse_named("NEWH640R.RES", &bytes).expect("NEWH640R.RES parse");
    assert_eq!(res.images.len(), 13, "NEWH640R.RES image count");
    assert_eq!((res.images[0].width, res.images[0].height), (640, 480));
    assert_eq!((res.images[1].width, res.images[1].height), (100, 100));
    assert_eq!((res.images[3].width, res.images[3].height), (56, 65));
}

#[test]
fn menu800_and_newh800_real_resource() {
    for (name, count) in [("MENU800R.RES", 15usize), ("NEWH800R.RES", 13usize)] {
        let path = install_dir().join(name);
        let Ok(bytes) = std::fs::read(&path) else {
            eprintln!("skipping: {} not present", path.display());
            continue;
        };
        let res = MenuRes::parse_named(name, &bytes).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(res.images.len(), count, "{name} image count");
        assert_eq!(res.images[0].width, 800);
        assert_eq!(res.images[0].height, 600);
    }
}

#[test]
fn rgb555_expansion_matches_expected() {
    assert_eq!(rgb555_to_rgba(0x7C00), [255, 0, 0, 255]);
    assert_eq!(rgb555_to_rgba(0x03E0), [0, 255, 0, 255]);
    assert_eq!(rgb555_to_rgba(0x001F), [0, 0, 255, 255]);
    assert_eq!(rgb555_to_rgba(0x7FFF), [255, 255, 255, 255]);
    assert_eq!(rgb555_to_rgba(0x0000), [0, 0, 0, 255]);
}
