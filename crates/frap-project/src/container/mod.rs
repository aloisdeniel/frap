// SPDX-License-Identifier: GPL-3.0-or-later
//! Deterministic zip: entry order, 1980-01-01 timestamps, fixed Deflate level, `mimetype` stored first (WP 1.9).
//!
//! ```text
//! scene.frap  (zip; entries in this exact order; all timestamps 1980-01-01 00:00:00)
//! ├─ mimetype                     "application/vnd.frap+zip"            (Stored, first)
//! ├─ manifest.json                pretty-printed, Deflated
//! ├─ scene.json                   pretty-printed, Deflated
//! ├─ buffers/<sha256-32hex>.bin   little-endian raw arrays, Deflated, hash order
//! └─ images/<sha256-32hex>.<ext>  original encoded bytes, Stored, (hash, ext) order
//! ```
//!
//! Every byte of the archive is a function of its inputs: no clock, no platform (`version made
//! by` is Unix, permissions 0644, no extra fields), and Deflate is zlib-rs at a fixed level
//! (its hash function depends only on the level, never on CPU features). Saving the same state
//! twice is byte-identical.
//!
//! Reading checks the `mimetype` entry, refuses newer `schema_version`s, runs the migrations
//! (`crate::migrate`) on the JSON trees, verifies every listed buffer and image against its
//! hash and size, and (unless lenient) rejects unknown entries and manifest fields. The typed
//! mapping of `scene.json` to `Main` is `crate::scene_map` (WP 1.23).

use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};

use serde_json::Value;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, System, ZipArchive, ZipWriter};

use crate::buffers::{BufferError, BufferStore, ContentHash, ImageRef, ImageStore, buffer_path};
use crate::manifest::{MIMETYPE, Manifest, ManifestError};
use crate::migrate::{self, MigrateError, MigrationReport};

mod pretty;
pub use pretty::{layout, to_json_pretty};

/// First entry: the media type, Stored, so `file(1)` and tools can sniff it at offset 38.
pub const ENTRY_MIMETYPE: &str = "mimetype";
pub const ENTRY_MANIFEST: &str = "manifest.json";
pub const ENTRY_SCENE: &str = "scene.json";
/// zlib level for Deflated entries. Changing it changes every saved file: do not.
pub const DEFLATE_LEVEL: i64 = 6;
/// Upper bound for `manifest.json` (it only lists entries).
const MAX_MANIFEST_BYTES: u64 = 64 << 20;
/// Upper bound for `scene.json` (large arrays live in buffers).
const MAX_SCENE_BYTES: u64 = 2 << 30;

/// Errors of reading and writing `.frap` files.
#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("not a .frap file: {0}")]
    NotFrap(String),
    #[error("corrupt .frap file: {0}")]
    Corrupt(String),
    #[error("corrupt .frap file: missing entry `{0}`")]
    MissingEntry(String),
    #[error("unexpected entry `{0}` in .frap file (open with --lenient to ignore it)")]
    UnexpectedEntry(String),
    #[error("invalid JSON in {entry}: {message}")]
    Json {
        entry: &'static str,
        message: String,
    },
    #[error(transparent)]
    Manifest(#[from] ManifestError),
    #[error(transparent)]
    Migrate(#[from] MigrateError),
    #[error(transparent)]
    Buffer(#[from] BufferError),
}

impl ProjectError {
    /// Whether the file was written by a newer frap (its `schema_version` is above this build's).
    #[must_use]
    pub fn is_newer_schema(&self) -> bool {
        matches!(self, Self::Migrate(MigrateError::Newer { .. }))
    }
}

impl From<zip::result::ZipError> for ProjectError {
    fn from(e: zip::result::ZipError) -> Self {
        match e {
            zip::result::ZipError::Io(e) => Self::Io(e),
            other => Self::Corrupt(other.to_string()),
        }
    }
}

/// Options of [`read`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReadOptions {
    /// Ignore unknown entries and unknown manifest fields (`--lenient`).
    pub lenient: bool,
}

/// A read `.frap`: the migrated, current-schema JSON trees and the verified entries.
#[derive(Debug, Clone)]
pub struct Loaded {
    pub manifest: Manifest,
    /// `scene.json`, migrated to the current schema (a JSON object).
    pub scene: Value,
    pub buffers: BufferStore,
    pub images: ImageStore,
    /// Set when the file had an older `schema_version`.
    pub migration: Option<MigrationReport>,
}

fn options(method: CompressionMethod) -> SimpleFileOptions {
    let opts = SimpleFileOptions::default()
        .compression_method(method)
        .last_modified_time(DateTime::default())
        .system(System::Unix)
        .unix_permissions(0o644)
        .large_file(false);
    if method == CompressionMethod::Deflated {
        opts.compression_level(Some(DEFLATE_LEVEL))
    } else {
        opts
    }
}

/// Writes a `.frap` archive: `scene_json` (already serialized, e.g. with [`to_json_pretty`]),
/// every buffer of `buffers` and every image of `images`. The manifest is derived from them.
pub fn write(
    scene_json: &[u8],
    buffers: &BufferStore,
    images: &ImageStore,
) -> Result<Vec<u8>, ProjectError> {
    write_with_generator(scene_json, buffers, images, &crate::manifest::generator())
}

/// [`write`] with an explicit `generator` (tests pin bytes independently of the crate version).
fn write_with_generator(
    scene_json: &[u8],
    buffers: &BufferStore,
    images: &ImageStore,
    generator: &str,
) -> Result<Vec<u8>, ProjectError> {
    match serde_json::from_slice::<Value>(scene_json) {
        Ok(Value::Object(_)) => {}
        Ok(_) => {
            return Err(ProjectError::Json {
                entry: ENTRY_SCENE,
                message: "top level is not an object".into(),
            });
        }
        Err(e) => {
            return Err(ProjectError::Json {
                entry: ENTRY_SCENE,
                message: e.to_string(),
            });
        }
    }
    let mut manifest = Manifest::new(buffers, images);
    generator.clone_into(&mut manifest.generator);
    let stored = options(CompressionMethod::Stored);
    let deflated = options(CompressionMethod::Deflated);

    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let mut entry =
        |name: &str, opts: SimpleFileOptions, bytes: &[u8]| -> Result<(), ProjectError> {
            zip.start_file(name, opts)?;
            zip.write_all(bytes)?;
            Ok(())
        };
    entry(ENTRY_MIMETYPE, stored, MIMETYPE.as_bytes())?;
    entry(ENTRY_MANIFEST, deflated, &manifest.to_json())?;
    entry(ENTRY_SCENE, deflated, scene_json)?;
    for (hash, bytes) in buffers.iter() {
        entry(&buffer_path(hash), deflated, bytes)?;
    }
    for (r, bytes) in images.iter() {
        entry(&r.path(), stored, bytes)?;
    }
    Ok(zip.finish()?.into_inner())
}

/// What an entry name is, if it is one a `.frap` may contain.
enum EntryKind {
    Mimetype,
    Manifest,
    Scene,
    Buffer(ContentHash),
    Image(ImageRef),
}

fn classify(name: &str) -> Option<EntryKind> {
    match name {
        ENTRY_MIMETYPE => return Some(EntryKind::Mimetype),
        ENTRY_MANIFEST => return Some(EntryKind::Manifest),
        ENTRY_SCENE => return Some(EntryKind::Scene),
        _ => {}
    }
    if let Some(rest) = name.strip_prefix("buffers/") {
        let hash = rest.strip_suffix(".bin")?.parse().ok()?;
        return Some(EntryKind::Buffer(hash));
    }
    let rest = name.strip_prefix("images/")?;
    let (hash, ext) = rest.split_once('.')?;
    let ext = crate::buffers::normalize_ext(ext)
        .ok()
        .filter(|e| e == ext)?;
    Some(EntryKind::Image(ImageRef {
        hash: hash.parse().ok()?,
        ext,
    }))
}

fn read_entry<R: Read + std::io::Seek>(
    zip: &mut ZipArchive<R>,
    name: &str,
    max: u64,
) -> Result<Vec<u8>, ProjectError> {
    let file = match zip.by_name(name) {
        Ok(f) => f,
        Err(zip::result::ZipError::FileNotFound) => {
            return Err(ProjectError::MissingEntry(name.to_owned()));
        }
        Err(e) => return Err(e.into()),
    };
    let size = file.size();
    if size > max {
        return Err(ProjectError::Corrupt(format!(
            "entry `{name}` is {size} bytes, more than the {max} allowed"
        )));
    }
    let mut out = Vec::with_capacity(usize::try_from(size).unwrap_or(0).min(64 << 20));
    file.take(size + 1)
        .read_to_end(&mut out)
        .map_err(|e| ProjectError::Corrupt(format!("entry `{name}`: {e}")))?;
    if out.len() as u64 != size {
        return Err(ProjectError::Corrupt(format!(
            "entry `{name}` holds {} bytes, its header says {size}",
            out.len()
        )));
    }
    Ok(out)
}

fn parse_json(entry: &'static str, bytes: &[u8]) -> Result<Value, ProjectError> {
    serde_json::from_slice(bytes).map_err(|e| ProjectError::Json {
        entry,
        message: e.to_string(),
    })
}

/// Reads a `.frap` archive (see the module docs for the checks).
pub fn read(bytes: &[u8], opts: ReadOptions) -> Result<Loaded, ProjectError> {
    let mut zip =
        ZipArchive::new(Cursor::new(bytes)).map_err(|e| ProjectError::NotFrap(e.to_string()))?;

    // `mimetype` must be the first entry, Stored, with the exact media type.
    {
        let not_frap = || {
            ProjectError::NotFrap(format!(
                "the first entry is not `{ENTRY_MIMETYPE}` = {MIMETYPE:?}"
            ))
        };
        if zip.is_empty() {
            return Err(not_frap());
        }
        let mut first = zip.by_index(0)?;
        if first.name()? != ENTRY_MIMETYPE
            || first.compression() != CompressionMethod::Stored
            || first.size() != MIMETYPE.len() as u64
        {
            return Err(not_frap());
        }
        let mut content = Vec::with_capacity(MIMETYPE.len());
        (&mut first)
            .take(MIMETYPE.len() as u64 + 1)
            .read_to_end(&mut content)?;
        if content != MIMETYPE.as_bytes() {
            return Err(not_frap());
        }
    }

    // Every other entry must be one a `.frap` may contain.
    let names: Vec<String> = zip
        .file_names()
        .map(|n| n.map(std::borrow::Cow::into_owned))
        .collect::<Result<_, _>>()?;
    let mut present_buffers = std::collections::BTreeSet::new();
    let mut present_images = std::collections::BTreeSet::new();
    for name in &names {
        match classify(name) {
            Some(EntryKind::Buffer(h)) => {
                present_buffers.insert(h);
            }
            Some(EntryKind::Image(r)) => {
                present_images.insert(r);
            }
            Some(EntryKind::Mimetype | EntryKind::Manifest | EntryKind::Scene) => {}
            None if opts.lenient => {}
            None => return Err(ProjectError::UnexpectedEntry(name.clone())),
        }
    }

    // Version check and migrations run on the raw JSON trees, before typed parsing.
    let mut manifest_value = parse_json(
        ENTRY_MANIFEST,
        &read_entry(&mut zip, ENTRY_MANIFEST, MAX_MANIFEST_BYTES)?,
    )?;
    let version = Manifest::schema_version_of(&manifest_value)?;
    migrate::check_supported(version)?;
    let mut scene = parse_json(
        ENTRY_SCENE,
        &read_entry(&mut zip, ENTRY_SCENE, MAX_SCENE_BYTES)?,
    )?;
    if !scene.is_object() {
        return Err(ProjectError::Json {
            entry: ENTRY_SCENE,
            message: "top level is not an object".into(),
        });
    }
    let migration = {
        let Value::Object(map) = &mut manifest_value else {
            return Err(ManifestError::NotAnObject.into());
        };
        migrate::migrate(version, map, &mut scene)?
    };
    let manifest = Manifest::from_value(manifest_value, opts.lenient)?;

    let mut buffers = BufferStore::new();
    for entry in &manifest.buffers {
        let path = buffer_path(&entry.hash);
        if !present_buffers.remove(&entry.hash) {
            return Err(ProjectError::MissingEntry(path));
        }
        let bytes = read_entry(&mut zip, &path, entry.size)?;
        if bytes.len() as u64 != entry.size {
            return Err(ProjectError::Corrupt(format!(
                "entry `{path}` holds {} bytes, manifest.json says {}",
                bytes.len(),
                entry.size
            )));
        }
        buffers.insert_verified(entry.hash, bytes, &path)?;
    }
    let mut images = ImageStore::new();
    for entry in &manifest.images {
        let r = entry.image_ref();
        let path = r.path();
        if !present_images.remove(&r) {
            return Err(ProjectError::MissingEntry(path));
        }
        let bytes = read_entry(&mut zip, &path, entry.size)?;
        if bytes.len() as u64 != entry.size {
            return Err(ProjectError::Corrupt(format!(
                "entry `{path}` holds {} bytes, manifest.json says {}",
                bytes.len(),
                entry.size
            )));
        }
        images.insert_verified(r, bytes, &path)?;
    }
    if !opts.lenient {
        if let Some(h) = present_buffers.first() {
            return Err(ProjectError::UnexpectedEntry(buffer_path(h)));
        }
        if let Some(r) = present_images.first() {
            return Err(ProjectError::UnexpectedEntry(r.path()));
        }
    }

    Ok(Loaded {
        manifest,
        scene,
        buffers,
        images,
        migration,
    })
}

/// Reads a `.frap` file.
pub fn read_file(path: &Path, opts: ReadOptions) -> Result<Loaded, ProjectError> {
    read(&std::fs::read(path)?, opts)
}

/// The temporary file [`save_file`] writes before renaming: `<path>@`, as Blender does for `.blend`.
#[must_use]
pub fn temp_path(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push("@");
    PathBuf::from(s)
}

/// Writes `bytes` to `path` atomically: to [`temp_path`] first, then renamed over `path`, so a
/// failed save never leaves a truncated `.frap` behind.
pub fn save_file(path: &Path, bytes: &[u8]) -> Result<(), ProjectError> {
    let tmp = temp_path(path);
    let result = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    Ok(result?)
}

#[cfg(test)]
mod tests;
