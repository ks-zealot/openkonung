//! Single-archive resource pack.
//!
//! The original install ships ~124 MB of separate data files (`Game.res`,
//! `GRAPH.RES`, `Objects.res`, `CLIENT.EXE`, `konung.res`, the menu/INTR RES
//! families, `QUEST_R.RES`, `SOUNDS.RES`, `Game.0/1/2`, 43 `Konung.N` maps and
//! the background-music Ogg tracks). The browser shell currently fetches each
//! one individually and hands it to the wasm loader.
//!
//! This module defines the **packed** representation: one ZIP archive whose
//! `bundle.bin` entry is a `bincode`-serialized [`ResourcePack`]. The wasm
//! runtime reads the ZIP, deserializes the Rust structure and populates the
//! same [`crate::Loader`] the raw path fills, so every downstream decoder is
//! unchanged.
//!
//! Packing is performed by the native-only `examples/pack.rs` target, which is
//! never compiled into the wasm module. If the archive is absent, unreadable or
//! invalid, the shell falls back to the original per-file downloads.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

pub mod zip;

/// Archive format version. Bump when the serialized layout changes.
pub const PACK_FORMAT: u32 = 3;

/// ZIP entry holding the serialized [`ResourcePack`].
pub const PACK_ENTRY: &str = "bundle.bin";

/// Human-readable index entry written alongside the bundle.
pub const PACK_INDEX: &str = "pack.json";

/// Raw `manifest.json` copied into the archive so later builds can regenerate
/// the final manifest without reading any original `.res`/`.exe` file.
pub const PACK_MANIFEST: &str = "manifest.raw.json";

/// Upper bound for the inflated `bundle.bin` payload. The shipped resources are
/// ~124 MB; this leaves generous headroom while rejecting zip bombs.
pub const MAX_BUNDLE_BYTES: usize = 384 * 1024 * 1024;

/// One named resource blob, tagged with the existing `KIND_*` ABI selector.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamedBlob {
    pub name: String,
    pub kind: u32,
    pub bytes: Vec<u8>,
}

/// One `Game.N` profile (`id` = suffix) or `Konung.N` map (`id` = location).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdBlob {
    pub id: u32,
    pub name: String,
    pub bytes: Vec<u8>,
}

/// One background-music track (Ogg Vorbis bytes).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MusicBlob {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// One browser-playable movie (transcoded H.264/AAC MP4 bytes).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoBlob {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// The complete serialized resource bundle.
///
/// `source` maps a stable logical key (for example `"graph"`) to the original
/// install filename (for example `"GRAPH.RES"`) purely for diagnostics; the
/// loader does not consume it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourcePack {
    pub format: u32,
    pub build: String,
    pub source: BTreeMap<String, String>,
    pub resources: Vec<NamedBlob>,
    pub profiles: Vec<IdBlob>,
    pub konungs: Vec<IdBlob>,
    pub light_maps: Vec<IdBlob>,
    pub music: Vec<MusicBlob>,
    pub videos: Vec<VideoBlob>,
}

/// A rejected or unreadable pack.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackError {
    pub message: String,
}

impl PackError {
    pub fn new(message: impl Into<String>) -> Self {
        PackError {
            message: message.into(),
        }
    }
}

impl fmt::Display for PackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "resource pack: {}", self.message)
    }
}

impl std::error::Error for PackError {}

impl From<bincode::Error> for PackError {
    fn from(e: bincode::Error) -> Self {
        PackError::new(e.to_string())
    }
}

impl From<std::io::Error> for PackError {
    fn from(e: std::io::Error) -> Self {
        PackError::new(e.to_string())
    }
}

/// Result of a successful [`ResourcePack::apply`]: metadata the shell needs
/// plus the music payloads (the Rust runtime itself does not play audio).
pub struct AppliedPack {
    pub build: String,
    pub music: Vec<MusicBlob>,
    pub videos: Vec<VideoBlob>,
}

impl ResourcePack {
    /// Serialize to the bytes stored in the ZIP's `bundle.bin` entry.
    pub fn to_bytes(&self) -> Result<Vec<u8>, PackError> {
        Ok(bincode::serialize(self)?)
    }

    /// Deserialize an inflated `bundle.bin` payload.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, PackError> {
        if bytes.len() > MAX_BUNDLE_BYTES {
            return Err(PackError::new(format!(
                "bundle is {} bytes, over the {MAX_BUNDLE_BYTES}-byte limit",
                bytes.len()
            )));
        }
        if bytes.is_empty() {
            return Err(PackError::new("empty bundle"));
        }
        let pack: ResourcePack = bincode::deserialize(bytes)?;
        Ok(pack)
    }

    /// Stable content id over the packed names and bytes. The native packer
    /// stores the result in `build`; the runtime only reports it.
    pub fn compute_build(&self) -> String {
        let mut h = Fnv::new();
        h.write_u32(PACK_FORMAT);
        for r in &self.resources {
            h.write_str(&r.name);
            h.write_u32(r.kind);
            h.write_bytes(&r.bytes);
        }
        for p in &self.profiles {
            h.write_u32(p.id);
            h.write_str(&p.name);
            h.write_bytes(&p.bytes);
        }
        for k in &self.konungs {
            h.write_u32(k.id);
            h.write_str(&k.name);
            h.write_bytes(&k.bytes);
        }
        for m in &self.light_maps {
            h.write_u32(m.id);
            h.write_str(&m.name);
            h.write_bytes(&m.bytes);
        }
        for m in &self.music {
            h.write_str(&m.name);
            h.write_bytes(&m.bytes);
        }
        for v in &self.videos {
            h.write_str(&v.name);
            h.write_bytes(&v.bytes);
        }
        format!("{:012x}", h.finish() & 0x0000_FFFF_FFFF_FFFF)
    }

    /// Structural validation. Never touches the live loader, so the runtime can
    /// reject a bad archive without disturbing the raw-fallback path.
    pub fn validate(&self) -> Result<(), PackError> {
        if self.format != PACK_FORMAT {
            return Err(PackError::new(format!(
                "unsupported format {} (expected {PACK_FORMAT})",
                self.format
            )));
        }

        let mut kinds = [false; 17];
        for r in &self.resources {
            let k = r.kind as usize;
            if k >= kinds.len() {
                return Err(PackError::new(format!(
                    "resource '{}' has unknown kind {}",
                    r.name, r.kind
                )));
            }
            if kinds[k] {
                return Err(PackError::new(format!(
                    "duplicate resource kind {} ('{}')",
                    r.kind, r.name
                )));
            }
            if r.bytes.is_empty() {
                return Err(PackError::new(format!(
                    "resource '{}' (kind {}) is empty",
                    r.name, r.kind
                )));
            }
            kinds[k] = true;
        }

        // Core game databases + executable tables.
        for k in [0u32, 1, 2, 3] {
            if !kinds[k as usize] {
                return Err(PackError::new(format!(
                    "missing required resource kind {k}"
                )));
            }
        }
        // Frontend layouts (both original resolutions) and their fonts/names.
        for k in [4u32, 5, 6, 7, 8] {
            if !kinds[k as usize] {
                return Err(PackError::new(format!(
                    "missing required resource kind {k}"
                )));
            }
        }
        // At least one INTR interface archive (the game prefers 800).
        if !kinds[11] && !kinds[12] {
            return Err(PackError::new("missing INTR interface archive"));
        }
        if !kinds[16] {
            return Err(PackError::new("missing LIGHTS.RES resource"));
        }
        if self.light_maps.is_empty() {
            return Err(PackError::new("no LIGHTS.N maps"));
        }
        let mut light_ids = std::collections::BTreeSet::new();
        for m in &self.light_maps {
            if m.bytes.is_empty() {
                return Err(PackError::new(format!("light map '{}' is empty", m.name)));
            }
            if !light_ids.insert(m.id) {
                return Err(PackError::new(format!("duplicate light map id {}", m.id)));
            }
        }

        let mut profile_ids = [false; 3];
        for p in &self.profiles {
            if p.bytes.is_empty() {
                return Err(PackError::new(format!("profile '{}' is empty", p.name)));
            }
            match p.id {
                0..=2 => {
                    if profile_ids[p.id as usize] {
                        return Err(PackError::new(format!("duplicate profile id {}", p.id)));
                    }
                    profile_ids[p.id as usize] = true;
                }
                other => {
                    return Err(PackError::new(format!("unexpected profile id {other}")));
                }
            }
        }
        if !profile_ids.iter().all(|&b| b) {
            return Err(PackError::new("missing Game.0/1/2 profiles"));
        }

        if self.konungs.is_empty() {
            return Err(PackError::new("no Konung.N maps"));
        }
        let mut konung_ids = std::collections::BTreeSet::new();
        for k in &self.konungs {
            if k.bytes.is_empty() {
                return Err(PackError::new(format!("map '{}' is empty", k.name)));
            }
            if !konung_ids.insert(k.id) {
                return Err(PackError::new(format!("duplicate Konung id {}", k.id)));
            }
        }

        let mut video_names = std::collections::BTreeSet::new();
        for v in &self.videos {
            if v.name.is_empty() || v.bytes.is_empty() {
                return Err(PackError::new("empty video payload"));
            }
            if !video_names.insert(v.name.as_str()) {
                return Err(PackError::new(format!("duplicate video '{}'", v.name)));
            }
        }

        Ok(())
    }

    /// Validate, then atomically replace the live loader and return the music
    /// payloads + build id. On error the existing loader is left untouched.
    pub fn apply(self) -> Result<AppliedPack, PackError> {
        self.validate()?;
        let ResourcePack {
            build,
            resources,
            profiles,
            konungs,
            light_maps,
            music,
            videos,
            ..
        } = self;

        let mut loader = crate::Loader::default();
        for r in resources {
            let NamedBlob { kind, bytes, .. } = r;
            match kind {
                0 => loader.game = bytes,
                1 => loader.graph = bytes,
                2 => loader.objects = bytes,
                3 => loader.exe = bytes,
                4 => loader.names = bytes,
                5 => loader.menu640 = bytes,
                6 => loader.menu800 = bytes,
                7 => loader.newh640 = bytes,
                8 => loader.newh800 = bytes,
                9 => loader.save640 = bytes,
                10 => loader.save800 = bytes,
                11 => loader.intr640 = bytes,
                12 => loader.intr800 = bytes,
                13 => loader.quest = bytes,
                14 => loader.sounds = bytes,
                15 => loader.flames = bytes,
                16 => loader.lights = bytes,
                _ => unreachable!("validated above"),
            }
        }
        for p in profiles {
            loader.profiles.insert(p.id, p.bytes);
        }
        for k in konungs {
            loader.konungs.push((k.id, k.bytes));
        }
        for m in light_maps {
            loader.light_maps.insert(m.id, m.bytes);
        }
        *crate::loader_mut() = loader;

        Ok(AppliedPack {
            build,
            music,
            videos,
        })
    }
}

/// Small deterministic FNV-1a used only for the pack content id.
struct Fnv(u64);

impl Fnv {
    fn new() -> Self {
        Fnv(0xcbf2_9ce4_8422_2325)
    }
    fn write_bytes(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= b as u64;
            self.0 = self.0.wrapping_mul(0x100_0000_01b3);
        }
    }
    fn write_str(&mut self, s: &str) {
        self.write_u32(s.len() as u32);
        self.write_bytes(s.as_bytes());
    }
    fn write_u32(&mut self, v: u32) {
        self.write_bytes(&v.to_le_bytes());
    }
    fn finish(&self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests;
