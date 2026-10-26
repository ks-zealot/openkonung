//! Original Konung sound archive decoder (`SOUNDS.RES` / `Sounds_.res`).
//!
//! Both files share one container format (recovered from `konung.res`; the
//! interface loader `FUN_004392B4` opens `SOUNDS.RES` and streams blocks into
//! `0x81A990[id]` with sizes from `0x6213DC[id]`):
//!
//! ```text
//! u8  magic[4]        = 00 00 00 01
//! u32 header_word                      // unresolved (data-region-ish)
//! Entry index[1000]                     // 8 bytes each, little-endian
//!     u32 data_offset                   // relative to DATA_BASE
//!     u32 data_size
//! data_region[...]                      // raw signed 16-bit mono PCM
//! ```
//!
//! The index occupies `8..8008`; the data region therefore starts at
//! [`DATA_BASE`] = `8008`. A slot is unused when `data_offset == 0xFFFFFFFF`.
//! `SOUNDS.RES` (21 981 240 B) holds **391** clips; `Sounds_.res` (13 314 264 B)
//! holds **244** (partly different content) — the game opens `SOUNDS.RES`.
//!
//! Playback: DirectSound (`DSOUND.dll`/`DirectSoundCreate`). The engine init
//! `FUN_00429C0C` creates three secondary buffers at [`SAMPLE_RATES`]
//! (22050 / 19050 / 25050 Hz), mono, 16-bit; the playback routine
//! `FUN_0042A13C(id, ...)` locks a buffer, copies the clip's PCM and plays it.
//! A clip's byte length is `frames * 2`.

use crate::res::u32 as rd_u32;

/// 8-byte archive header.
pub const HEADER_SIZE: usize = 8;
/// Number of index slots.
pub const INDEX_ENTRIES: usize = 1000;
/// Index-table size in bytes (`1000 * 8`).
pub const INDEX_SIZE: usize = INDEX_ENTRIES * 8;
/// First byte of the PCM data region (`HEADER_SIZE + INDEX_SIZE`).
pub const DATA_BASE: usize = HEADER_SIZE + INDEX_SIZE;
/// Index-table marker for an unused slot.
pub const UNUSED: u32 = 0xFFFF_FFFF;
/// Container magic.
pub const MAGIC: [u8; 4] = [0x00, 0x00, 0x00, 0x01];
/// Output sample rates the original engine selects between (pitch variation).
pub const SAMPLE_RATES: [u32; 3] = [22050, 19050, 25050];
/// Bits per sample (signed PCM).
pub const BITS: u16 = 16;
/// Channels.
pub const CHANNELS: u16 = 1;
/// Bytes per frame (`BITS/8 * CHANNELS`).
pub const BYTES_PER_FRAME: usize = 2;

/// A malformed sound archive. Never partially parsed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoundError {
    pub message: String,
}

impl SoundError {
    fn new(message: impl Into<String>) -> Self {
        SoundError {
            message: message.into(),
        }
    }
}

impl core::fmt::Display for SoundError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "SOUNDS.RES: {}", self.message)
    }
}

impl std::error::Error for SoundError {}

/// A parsed sound archive; clips are slices into the owned bytes.
#[derive(Clone, Debug)]
pub struct SoundArchive {
    bytes: Vec<u8>,
    entries: Vec<Option<(u32, u32)>>,
}

impl SoundArchive {
    /// Parse and validate a `SOUNDS.RES`/`Sounds_.res` image.
    pub fn parse(bytes: &[u8]) -> Result<SoundArchive, SoundError> {
        if bytes.len() < DATA_BASE {
            return Err(SoundError::new(format!(
                "file too small ({} bytes, need at least {DATA_BASE})",
                bytes.len()
            )));
        }
        if bytes[0..4] != MAGIC {
            return Err(SoundError::new(format!(
                "bad magic {:02X} {:02X} {:02X} {:02X}",
                bytes[0], bytes[1], bytes[2], bytes[3]
            )));
        }
        let mut entries: Vec<Option<(u32, u32)>> = Vec::with_capacity(INDEX_ENTRIES);
        for i in 0..INDEX_ENTRIES {
            let o = HEADER_SIZE + i * 8;
            let off = rd_u32(bytes, o);
            let size = rd_u32(bytes, o + 4);
            if off == UNUSED || size == UNUSED || size == 0 {
                entries.push(None);
                continue;
            }
            let start = DATA_BASE as u64 + off as u64;
            let end = start + size as u64;
            if end > bytes.len() as u64 {
                return Err(SoundError::new(format!(
                    "index {i}: block [{start}..{end}) exceeds file ({} bytes)",
                    bytes.len()
                )));
            }
            entries.push(Some((off, size)));
        }
        Ok(SoundArchive {
            bytes: bytes.to_vec(),
            entries,
        })
    }

    /// Number of occupied clips.
    pub fn len(&self) -> usize {
        self.entries.iter().filter(|e| e.is_some()).count()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.iter().all(|e| e.is_none())
    }

    /// Raw `(data_offset, data_size)` for `id`, or `None` when unused/out of range.
    pub fn entry(&self, id: usize) -> Option<(u32, u32)> {
        self.entries.get(id).copied().flatten()
    }

    /// Raw signed 16-bit little-endian mono PCM bytes for `id`.
    pub fn clip_bytes(&self, id: usize) -> Option<&[u8]> {
        let (off, size) = self.entry(id)?;
        let start = DATA_BASE + off as usize;
        Some(&self.bytes[start..start + size as usize])
    }

    /// Number of 16-bit mono frames in clip `id`.
    pub fn frame_count(&self, id: usize) -> Option<usize> {
        self.entry(id)
            .map(|(_, size)| size as usize / BYTES_PER_FRAME)
    }

    /// Playback duration in milliseconds at the archive's base rate.
    pub fn duration_ms(&self, id: usize, rate: u32) -> Option<u64> {
        let frames = self.frame_count(id)?;
        if rate == 0 {
            return None;
        }
        Some(frames as u64 * 1000 / rate as u64)
    }
}

pub mod events;
pub mod position;

pub use position::*;

/// One queued `FUN_0042A13C` playback request.
///
/// `volume` and `pan` are the original hundredths-of-a-decibel / -10000..10000
/// arguments (`0` = full volume, centred); `rate` is the sample rate of the
/// secondary buffer the original would have copied the clip into.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SoundEvent {
    pub id: u16,
    pub volume: i32,
    pub pan: i32,
    pub rate: u32,
}

impl SoundEvent {
    /// Non-positional request at buffer 0 (the original call sites that pass
    /// `volume = 0`, `pan = 0`).
    pub fn centred(id: u16) -> SoundEvent {
        SoundEvent {
            id,
            volume: 0,
            pan: 0,
            rate: SAMPLE_RATES[0],
        }
    }
}

impl From<u16> for SoundEvent {
    fn from(id: u16) -> Self {
        SoundEvent::centred(id)
    }
}

#[cfg(test)]
mod tests;
