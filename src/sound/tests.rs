//! Unit tests for the [`crate::sound`] decoder.

use super::*;

fn put_u32(b: &mut [u8], o: usize, v: u32) {
    b[o..o + 4].copy_from_slice(&v.to_le_bytes());
}

/// Minimal well-formed archive with one clip of `frames` s16 mono samples.
fn synth(frames: usize) -> Vec<u8> {
    let size = frames * BYTES_PER_FRAME;
    let mut b = vec![0u8; DATA_BASE + size];
    b[0..4].copy_from_slice(&MAGIC);
    put_u32(&mut b, 4, 0x1122_3344);
    // slot 7 = clip at offset 0
    put_u32(&mut b, HEADER_SIZE + 7 * 8, 0);
    put_u32(&mut b, HEADER_SIZE + 7 * 8 + 4, size as u32);
    for i in 0..frames {
        let v = (i as i16).wrapping_mul(3);
        b[DATA_BASE + i * 2..DATA_BASE + i * 2 + 2].copy_from_slice(&v.to_le_bytes());
    }
    b
}

#[test]
fn parses_synthetic_clip() {
    let a = SoundArchive::parse(&synth(100)).unwrap();
    assert_eq!(a.len(), 1);
    assert_eq!(a.entry(7), Some((0, 200)));
    assert_eq!(a.frame_count(7), Some(100));
    assert_eq!(a.clip_bytes(7).unwrap().len(), 200);
    assert_eq!(a.entry(0), None);
    assert_eq!(a.entry(999), None);
}

#[test]
fn rejects_bad_magic_and_truncation() {
    let mut b = synth(10);
    b[0] = 0xFF;
    assert!(SoundArchive::parse(&b).is_err());
    assert!(SoundArchive::parse(&synth(10)[..DATA_BASE - 1]).is_err());

    // Block extending past EOF is rejected.
    let mut b = synth(10);
    put_u32(&mut b, HEADER_SIZE + 7 * 8 + 4, 0xFFFF);
    assert!(SoundArchive::parse(&b).is_err());
}

/// Shipped `SOUNDS.RES`, gated on file presence.
#[test]
fn decodes_shipped_sounds() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let Some(data) = std::fs::read(dir.join("SOUNDS.RES")).ok() else {
        eprintln!("skipping: SOUNDS.RES not present");
        return;
    };
    let a = SoundArchive::parse(&data).unwrap();
    assert_eq!(a.len(), 391, "SOUNDS.RES occupied clip count");
    assert_eq!(a.entry(0), Some((0, 66298)));
    assert_eq!(a.frame_count(0), Some(66298 / 2));
    // NPC voice ids 0x0C..0x13 must be present and 16-bit aligned.
    for id in 0x0Cu32 as usize..=0x13 {
        let (_, size) = a
            .entry(id)
            .unwrap_or_else(|| panic!("voice id {id} missing"));
        assert_eq!(
            size % BYTES_PER_FRAME as u32,
            0,
            "id {id} not frame-aligned"
        );
    }
    assert!(a.duration_ms(12, SAMPLE_RATES[0]).unwrap() > 0);
}
