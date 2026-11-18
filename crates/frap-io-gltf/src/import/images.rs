// SPDX-License-Identifier: GPL-3.0-or-later
//! glTF images → `bpy.data.images`: embedded images (buffer views, data URIs) are packed; files
//! next to the glTF are loaded (`images.load(check_existing=True)`) and packed when
//! `import_pack_images`; a missing file gives a placeholder and an ERROR report.
//!
//! Ported from io_scene_gltf2 5.2.41 `blender/imp/image.py` (`BlenderImage.create`,
//! `create_from_file`, `create_from_data`, `_placeholder_image`) and `texture.py`
//! (`get_source`), Apache-2.0, The glTF-Blender-IO authors.

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use frap_gltf::accessor::{buffer_view_bytes, decode_data_uri, uri_to_path};
use frap_rna::{IdRef, ImageId, ReportType};
use frap_scene::id::{EncodedImage, Image};
use frap_scene::{ChangeLog, Main};
use smol_str::SmolStr;

use super::{Gltf, ImportError};

/// `get_source`: the image of a texture (the EXT_texture_webp source when preferred or when
/// the core source is missing).
pub(crate) fn texture_source(cx: &Gltf, texture: usize) -> Option<usize> {
    let tex = cx.doc.textures.get(texture)?;
    let webp = tex
        .extensions
        .get_raw("EXT_texture_webp")
        .and_then(|v| v.get("source"))
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    let src = tex.source.map(|s| s as usize);
    if cx.settings.import_webp_texture {
        webp.or(src)
    } else {
        src.or(webp)
    }
}

/// `os.path.abspath`: absolute and lexically normalised (`.` and `..` removed).
fn abspath(p: &Path) -> PathBuf {
    let abs = if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(p)
    };
    let mut out = PathBuf::new();
    for c in abs.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            c => out.push(c.as_os_str()),
        }
    }
    out
}

/// `bpy.path.relpath(path)` against the saved `.blend` (`//` prefix); `None` when there is
/// no common root (another Windows drive).
fn relpath(path: &Path, blend: &Path) -> Option<String> {
    let base = abspath(blend.parent()?);
    let path = abspath(path);
    let (a, b): (Vec<_>, Vec<_>) = (base.components().collect(), path.components().collect());
    if a.first() != b.first() {
        return None;
    }
    let common = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let mut rel = String::from("//");
    for _ in common..a.len() {
        rel.push_str("../");
    }
    let rest: Vec<String> = b[common..]
        .iter()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    rel.push_str(&rest.join("/"));
    Some(rel)
}

/// MIME type of encoded image bytes.
fn mime_of(bytes: &[u8], declared: Option<&str>) -> SmolStr {
    if let Some(m) = declared {
        return m.into();
    }
    match frap_imbuf::FileFormat::detect(bytes) {
        Some(frap_imbuf::FileFormat::Png) => "image/png".into(),
        Some(frap_imbuf::FileFormat::Jpeg) => "image/jpeg".into(),
        Some(frap_imbuf::FileFormat::WebP) => "image/webp".into(),
        _ => "application/octet-stream".into(),
    }
}

/// Fills the file metadata of an image whose encoded bytes are known (`file_format`, `size`).
fn set_file_metadata(img: &mut Image, bytes: &[u8]) {
    if let Some(f) = frap_imbuf::FileFormat::detect(bytes) {
        img.file_format = f.identifier().into();
    }
    img.size = match frap_imbuf::decode(bytes) {
        Ok(d) => [d.width as i32, d.height as i32],
        Err(_) => [0, 0],
    };
}

/// `BlenderImage.create`: the Blender image of glTF image `idx` (created once).
pub(crate) fn create(
    cx: &mut Gltf,
    main: &mut Main,
    log: &mut ChangeLog,
    idx: usize,
) -> Result<Option<IdRef<ImageId>>, ImportError> {
    if let Some(r) = cx.images.get(&idx) {
        return Ok(*r);
    }
    let Some(pyimg) = cx.doc.images.get(idx) else {
        return Ok(None);
    };
    let r = match pyimg.uri.as_deref() {
        Some(uri) if !uri.starts_with("data:") => Some(create_from_file(cx, main, log, idx)?),
        _ => create_from_data(cx, main, log, idx)?,
    };
    if let Some(r) = r
        && let Some(img) = main.images.get_mut(r, log)
    {
        img.alpha_mode = "CHANNEL_PACKED".into();
    }
    cx.images.insert(idx, r);
    Ok(r)
}

fn create_from_file(
    cx: &mut Gltf,
    main: &mut Main,
    log: &mut ChangeLog,
    idx: usize,
) -> Result<IdRef<ImageId>, ImportError> {
    let pyimg = &cx.doc.images[idx];
    let uri = pyimg.uri.clone().unwrap_or_default();
    let abs = abspath(&cx.dir.join(uri_to_path(&uri)));
    let mut path = abs.to_string_lossy().into_owned();
    if let Some(blend) = main.filepath.as_deref()
        && let Some(rel) = relpath(&abs, Path::new(blend))
    {
        path = rel;
    }
    let basename = abs
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    // `img.name or basename(path)`
    let img_name = super::truthy_name(&pyimg.name).unwrap_or(basename);
    let declared_mime = pyimg.mime_type.clone();
    match std::fs::read(&abs) {
        Ok(bytes) => {
            // check_existing=True: an image already loaded from the same file is reused.
            let existing = main
                .images
                .iter()
                .find(|(_, im)| im.source == "FILE" && abspath(Path::new(&im.filepath)) == abs)
                .map(|(r, _)| r);
            if let Some(r) = existing {
                if cx.settings.import_pack_images
                    && let Some(im) = main.images.get_mut(r, log)
                    && im.packed_file.is_none()
                {
                    im.packed_file = Some(Arc::new(EncodedImage {
                        mime: mime_of(&bytes, declared_mime.as_deref()),
                        bytes,
                    }));
                }
                return Ok(r);
            }
            let mut img = Image::new("", 0, 0);
            img.source = "FILE".into();
            img.filepath = path;
            set_file_metadata(&mut img, &bytes);
            if cx.settings.import_pack_images {
                img.packed_file = Some(Arc::new(EncodedImage {
                    mime: mime_of(&bytes, declared_mime.as_deref()),
                    bytes,
                }));
            }
            Ok(main.images.insert(&img_name, img, log))
        }
        Err(_) => {
            cx.report(
                ReportType::Error,
                format!("Missing image file (index {idx}): {}", abs.display()),
            );
            let mut img = Image::new("", 128, 128);
            img.filepath = abs.to_string_lossy().into_owned();
            img.source = "FILE".into();
            img.size = [0, 0];
            Ok(main.images.insert(&img_name, img, log))
        }
    }
}

fn create_from_data(
    cx: &mut Gltf,
    main: &mut Main,
    log: &mut ChangeLog,
    idx: usize,
) -> Result<Option<IdRef<ImageId>>, ImportError> {
    let pyimg = &cx.doc.images[idx];
    let bytes = if let Some(uri) = pyimg.uri.as_deref() {
        match decode_data_uri(uri) {
            Some(r) => r?.1,
            None => return Ok(None),
        }
    } else if let Some(view) = pyimg.buffer_view {
        buffer_view_bytes(&cx.doc, &cx.buffers, view)?.to_vec()
    } else {
        return Ok(None);
    };
    // `gltf.data.images[img_idx].name or 'Image_%d'`
    let name = super::truthy_name(&pyimg.name).unwrap_or_else(|| format!("Image_{idx}"));
    let mut img = Image::new("", 8, 8);
    img.source = "FILE".into();
    set_file_metadata(&mut img, &bytes);
    img.packed_file = Some(Arc::new(EncodedImage {
        mime: mime_of(&bytes, pyimg.mime_type.as_deref()),
        bytes,
    }));
    Ok(Some(main.images.insert(&name, img, log)))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn paths() {
        assert_eq!(
            abspath(Path::new("/a/b/../c/./d.png")),
            PathBuf::from("/a/c/d.png")
        );
        assert_eq!(
            relpath(Path::new("/a/c/d.png"), Path::new("/a/b/x.frap")).as_deref(),
            Some("//../c/d.png")
        );
        assert_eq!(
            relpath(Path::new("/a/b/t/d.png"), Path::new("/a/b/x.frap")).as_deref(),
            Some("//t/d.png")
        );
    }
}
