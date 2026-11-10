//! Minimal ZIP reader for the resource pack, plus a native-only writer.
//!
//! The wasm runtime only ever needs one entry (`bundle.bin`) from a standard
//! ZIP archive produced by `examples/pack.rs`. A full ZIP implementation would
//! add unnecessary code to the wasm module, so the reader supports exactly the
//! subset the packer emits:
//!
//! - STORE (method 0) and DEFLATE (method 8),
//! - no encryption, no ZIP64, no multi-disk archives,
//! - one central directory at the end of the file.
//!
//! CRC-32 is verified for every entry. Size and entry-count limits are enforced
//! against malicious/broken archives.
//!
//! Structures read (all little-endian; field offsets used below):
//!
//! ```text
//! end of central directory  0x06054B50, 22 bytes + comment
//!                           +4 disk, +8 entries on disk, +10 entries,
//!                           +12 directory size, +16 directory offset
//! central directory entry   0x02014B50, 46 bytes + name/extra/comment
//!                           +8 flags, +10 method, +16 crc-32,
//!                           +20 compressed size, +24 uncompressed size,
//!                           +28 name len, +30 extra len, +32 comment len,
//!                           +42 local header offset
//! local file header         0x04034B50, 30 bytes + name/extra
//!                           +26 name len, +28 extra len
//! ```

use std::io::Read;

use super::{PackError, MAX_BUNDLE_BYTES};

const EOCD_SIG: u32 = 0x0605_4b50;
const CENTRAL_SIG: u32 = 0x0201_4b50;
const LOCAL_SIG: u32 = 0x0403_4b50;
const EOCD_MIN_SIZE: usize = 22;
const MAX_COMMENT: usize = 65_535;

/// Build a [`PackError`] from a reason string (no prefix; `Display` adds it).
fn err(message: impl Into<String>) -> PackError {
    PackError::new(message)
}

/// Little-endian `u16` at `o`, or `None` when fewer than two bytes remain.
fn rd_u16(b: &[u8], o: usize) -> Option<u16> {
    if o + 2 > b.len() {
        return None;
    }
    Some((b[o] as u16) | ((b[o + 1] as u16) << 8))
}

/// Little-endian `u32` at `o`, or `None` when fewer than four bytes remain.
fn rd_u32(b: &[u8], o: usize) -> Option<u32> {
    if o + 4 > b.len() {
        return None;
    }
    Some(
        (b[o] as u32)
            | ((b[o + 1] as u32) << 8)
            | ((b[o + 2] as u32) << 16)
            | ((b[o + 3] as u32) << 24),
    )
}

/// Locate the end-of-central-directory record by scanning backwards.
///
/// The signature `0x06054B50` is only accepted when its declared comment length
/// still fits inside the archive, and the scan cannot retreat further than
/// `22 + 65535` bytes from the end (the largest legal EOCD), so a buffer full of
/// signature-like bytes is rejected instead of searched forever.
fn find_eocd(zip: &[u8]) -> Result<usize, PackError> {
    if zip.len() < EOCD_MIN_SIZE {
        return Err(err("archive too small"));
    }
    let min = zip.len().saturating_sub(EOCD_MIN_SIZE + MAX_COMMENT);
    let mut i = zip.len() - EOCD_MIN_SIZE;
    loop {
        if rd_u32(zip, i) == Some(EOCD_SIG) {
            let comment = rd_u16(zip, i + 20).ok_or_else(|| err("truncated EOCD"))? as usize;
            if i + EOCD_MIN_SIZE + comment <= zip.len() {
                return Ok(i);
            }
        }
        if i == min {
            return Err(err("end-of-central-directory record not found"));
        }
        i -= 1;
    }
}

/// Read and inflate one archive entry by its exact name.
pub fn read_entry(zip: &[u8], wanted: &str) -> Result<Vec<u8>, PackError> {
    read_entry_limited(zip, wanted, MAX_BUNDLE_BYTES)
}

/// Like [`read_entry`] with an explicit uncompressed-size limit.
pub fn read_entry_limited(zip: &[u8], wanted: &str, max_size: usize) -> Result<Vec<u8>, PackError> {
    if wanted.is_empty() || wanted.len() > 255 {
        return Err(err("invalid entry name"));
    }
    let eocd = find_eocd(zip)?;

    let disk = rd_u16(zip, eocd + 4).ok_or_else(|| err("truncated EOCD"))?;
    let cd_disk = rd_u16(zip, eocd + 6).ok_or_else(|| err("truncated EOCD"))?;
    let entries_disk = rd_u16(zip, eocd + 8).ok_or_else(|| err("truncated EOCD"))?;
    let entries = rd_u16(zip, eocd + 10).ok_or_else(|| err("truncated EOCD"))?;
    let cd_size = rd_u32(zip, eocd + 12).ok_or_else(|| err("truncated EOCD"))? as usize;
    let cd_off = rd_u32(zip, eocd + 16).ok_or_else(|| err("truncated EOCD"))? as usize;

    if disk != 0 || cd_disk != 0 || entries_disk != entries {
        return Err(err("multi-disk archives are not supported"));
    }
    if entries == u16::MAX || cd_size == u32::MAX as usize || cd_off == u32::MAX as usize {
        return Err(err("ZIP64 archives are not supported"));
    }
    if cd_off
        .checked_add(cd_size)
        .map_or(true, |end| end > zip.len())
    {
        return Err(err("central directory out of bounds"));
    }
    if cd_off > eocd {
        return Err(err("central directory overlaps EOCD"));
    }

    let mut p = cd_off;
    for _ in 0..entries {
        if rd_u32(zip, p) != Some(CENTRAL_SIG) {
            return Err(err("bad central-directory signature"));
        }
        let flags = rd_u16(zip, p + 8).ok_or_else(|| err("truncated directory"))?;
        let method = rd_u16(zip, p + 10).ok_or_else(|| err("truncated directory"))?;
        let crc = rd_u32(zip, p + 16).ok_or_else(|| err("truncated directory"))?;
        let csize = rd_u32(zip, p + 20).ok_or_else(|| err("truncated directory"))? as usize;
        let usize_ = rd_u32(zip, p + 24).ok_or_else(|| err("truncated directory"))? as usize;
        let nlen = rd_u16(zip, p + 28).ok_or_else(|| err("truncated directory"))? as usize;
        let elen = rd_u16(zip, p + 30).ok_or_else(|| err("truncated directory"))? as usize;
        let clen = rd_u16(zip, p + 32).ok_or_else(|| err("truncated directory"))? as usize;
        let local_off = rd_u32(zip, p + 42).ok_or_else(|| err("truncated directory"))? as usize;
        let next = p + 46 + nlen + elen + clen;
        if next > zip.len() || p + 46 + nlen > zip.len() {
            return Err(err("central directory entry out of bounds"));
        }
        let name = &zip[p + 46..p + 46 + nlen];

        if name == wanted.as_bytes() {
            if flags & 1 != 0 {
                return Err(err("encrypted entries are not supported"));
            }
            if usize_ > max_size {
                return Err(err(format!(
                    "entry '{wanted}' is {usize_} bytes, over the {max_size}-byte limit"
                )));
            }
            return read_local(zip, wanted, local_off, method, crc, csize, usize_);
        }
        p = next;
    }

    Err(err(format!("entry '{wanted}' not found")))
}

/// Read the local header of the matched entry and return its verified payload.
///
/// Checks the local signature, that the local name equals `wanted` (so a
/// central directory cannot redirect to another entry), and the data bounds.
/// Method `0` requires `compressed == uncompressed`; method `8` inflates through
/// a `take(uncompressed + 1)` limiter so a lying size field cannot balloon in
/// memory. The CRC-32 of the result must equal the central-directory value.
fn read_local(
    zip: &[u8],
    wanted: &str,
    local_off: usize,
    method: u16,
    crc: u32,
    csize: usize,
    usize_: usize,
) -> Result<Vec<u8>, PackError> {
    if rd_u32(zip, local_off) != Some(LOCAL_SIG) {
        return Err(err("bad local-header signature"));
    }
    let nlen = rd_u16(zip, local_off + 26).ok_or_else(|| err("truncated local header"))? as usize;
    let elen = rd_u16(zip, local_off + 28).ok_or_else(|| err("truncated local header"))? as usize;
    if local_off + 30 + nlen > zip.len()
        || &zip[local_off + 30..local_off + 30 + nlen] != wanted.as_bytes()
    {
        return Err(err("local header name mismatch"));
    }
    let data_off = local_off
        .checked_add(30 + nlen + elen)
        .ok_or_else(|| err("local header offset overflow"))?;
    let data_end = data_off
        .checked_add(csize)
        .ok_or_else(|| err("entry size overflow"))?;
    if data_end > zip.len() {
        return Err(err("entry data out of bounds"));
    }
    let data = &zip[data_off..data_end];

    let out = match method {
        0 => {
            if csize != usize_ {
                return Err(err("stored entry size mismatch"));
            }
            data.to_vec()
        }
        8 => {
            let mut out = Vec::with_capacity(usize_.min(MAX_BUNDLE_BYTES));
            let decoder = flate2::read::DeflateDecoder::new(data);
            let mut limited = decoder.take(usize_ as u64 + 1);
            limited
                .read_to_end(&mut out)
                .map_err(|e| err(format!("inflate failed for '{wanted}': {e}")))?;
            if out.len() != usize_ {
                return Err(err(format!(
                    "entry '{wanted}' inflates to {} bytes, expected {usize_}",
                    out.len()
                )));
            }
            out
        }
        other => return Err(err(format!("unsupported compression method {other}"))),
    };

    if crc32fast::hash(&out) != crc {
        return Err(err(format!("CRC mismatch for '{wanted}'")));
    }
    Ok(out)
}

// --------------------------------------------------------------------------
// Native-only writer (used by examples/pack.rs and tests)
// --------------------------------------------------------------------------

/// Write a standard ZIP archive. `compress` selects DEFLATE (true) or STORE.
#[cfg(not(target_arch = "wasm32"))]
pub fn write_archive_bytes(entries: &[(&str, &[u8], bool)]) -> Result<Vec<u8>, PackError> {
    use std::io::Write;

    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let timestamp = zip::DateTime::from_date_and_time(1980, 1, 1, 0, 0, 0)
        .map_err(|e| err(format!("invalid fixed timestamp: {e}")))?;

    for &(name, data, compress) in entries {
        let method = if compress {
            zip::CompressionMethod::Deflated
        } else {
            zip::CompressionMethod::Stored
        };
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(method)
            .compression_level(if compress { Some(6) } else { None })
            .last_modified_time(timestamp);
        writer
            .start_file(name, options)
            .map_err(|e| err(format!("start '{name}': {e}")))?;
        writer
            .write_all(data)
            .map_err(|e| err(format!("write '{name}': {e}")))?;
    }

    let cursor = writer
        .finish()
        .map_err(|e| err(format!("finish archive: {e}")))?;
    Ok(cursor.into_inner())
}

/// Write [`write_archive_bytes`] to a file, atomically via a sibling `.tmp`.
#[cfg(not(target_arch = "wasm32"))]
pub fn write_archive_file(
    entries: &[(&str, &[u8], bool)],
    path: &std::path::Path,
) -> Result<u64, PackError> {
    let bytes = write_archive_bytes(entries)?;
    let tmp = path.with_extension("zip.tmp");
    std::fs::write(&tmp, &bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(bytes.len() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Both a DEFLATE entry and a STORE entry round-trip byte-exactly.
    #[test]
    fn roundtrips_stored_and_deflated_entries() {
        let payload = b"hello openkonung".repeat(100);
        let stored = b"plain".to_vec();
        let zip = write_archive_bytes(&[
            ("bundle.bin", payload.as_slice(), true),
            ("pack.json", stored.as_slice(), false),
        ])
        .unwrap();

        assert_eq!(read_entry(&zip, "bundle.bin").unwrap(), payload);
        assert_eq!(read_entry(&zip, "pack.json").unwrap(), stored);
    }

    /// An absent entry name is an error, never an empty payload.
    #[test]
    fn rejects_missing_entry() {
        let zip = write_archive_bytes(&[("a", b"x", true)]).unwrap();
        let e = read_entry(&zip, "bundle.bin").unwrap_err();
        assert!(e.message.contains("not found"), "{}", e.message);
    }

    /// A corrupted payload byte is caught by the CRC-32 (or the size check).
    #[test]
    fn rejects_corrupt_crc() {
        let payload = b"hello openkonung".repeat(100);
        let mut zip = write_archive_bytes(&[("bundle.bin", payload.as_slice(), true)]).unwrap();
        // Flip a payload byte, not a header byte; locate the local data start.
        let nlen = u16::from_le_bytes([zip[26], zip[27]]) as usize;
        let elen = u16::from_le_bytes([zip[28], zip[29]]) as usize;
        let data = 30 + nlen + elen;
        // Skip the first byte so the deflate stream stays decodable but the CRC
        // or the size check fails.
        zip[data + 1] ^= 0xFF;
        assert!(read_entry(&zip, "bundle.bin").is_err());
    }

    /// Truncating the archive removes the EOCD and is rejected.
    #[test]
    fn rejects_truncated_archive() {
        let zip = write_archive_bytes(&[("bundle.bin", b"payload", true)]).unwrap();
        assert!(read_entry(&zip[..zip.len() / 2], "bundle.bin").is_err());
    }

    /// The declared uncompressed size is checked before any inflation happens.
    #[test]
    fn enforces_size_limit() {
        let zip = write_archive_bytes(&[("bundle.bin", b"0123456789", false)]).unwrap();
        let e = read_entry_limited(&zip, "bundle.bin", 4).unwrap_err();
        assert!(e.message.contains("limit"), "{}", e.message);
    }
}
