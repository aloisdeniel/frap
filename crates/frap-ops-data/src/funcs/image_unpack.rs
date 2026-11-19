// SPDX-License-Identifier: GPL-3.0-or-later
//! `Image.unpack(method='USE_LOCAL')` (`rna_Image_unpack` → `BKE_packedfile_unpack_image`,
//! `packedFile.cc`, Blender 5.2.4, GPL-2.0-or-later).
//!
//! The packed file is written to (or found at) the *local* path `//textures/<file name>` or
//! the *original* path (the packed file's own `filepath`); a packed file without a name is
//! named after the image plus the extension of its bytes (`Gen Img` → `Gen Img.png`), and one
//! without a folder uses `//`. Verified on 5.2.4 (`method`):
//! - `USE_LOCAL` (default) / `USE_ORIGINAL`: use the existing file if there is one (INFO
//!   `Use existing file (instead of packed): …` for the original), else write it;
//! - `WRITE_LOCAL` / `WRITE_ORIGINAL`: write it, replacing an existing file;
//! - `REMOVE`: drop the packed data, keep `filepath`.
//!
//! Writing reports INFO `Saved packed file to: <path>` and creates missing folders. Afterwards
//! the image is not packed and `filepath` is the path used (`//textures/u5.png`), except with
//! `REMOVE`. `//` is relative to the saved `.frap` file's folder, or to the working directory.
//! Errors: `Image not packed`, `Image is not editable` (linked), `Unpacking movies or image
//! sequences not supported`, `Error creating file '…'`.

use frap_rna::{IdRef, ImageId, RnaError, RnaPtr, Value, rna_func};
use frap_scene::id::{Image, PathBase};
use frap_scene::{FuncArgs, Host};

use super::_common::{arg_str, internal};
use super::image_save::write::{Reports, image_ref, write_file};

/// `ePF_FileStatus`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Method {
    /// `PF_REMOVE`
    Remove,
    /// `PF_USE_LOCAL`
    UseLocal,
    /// `PF_WRITE_LOCAL`
    WriteLocal,
    /// `PF_USE_ORIGINAL`
    UseOriginal,
    /// `PF_WRITE_ORIGINAL`
    WriteOriginal,
}

impl Method {
    /// From the RNA enum identifier (`rna_enum_unpack_method_items`).
    pub(crate) fn from_identifier(s: &str) -> Method {
        match s {
            "REMOVE" => Method::Remove,
            "WRITE_LOCAL" => Method::WriteLocal,
            "USE_ORIGINAL" => Method::UseOriginal,
            "WRITE_ORIGINAL" => Method::WriteOriginal,
            _ => Method::UseLocal,
        }
    }
}

/// The extension Blender gives packed bytes of a known format (`BKE_image_path_ext_from_imtype`).
fn extension_of(bytes: &[u8]) -> Option<&'static str> {
    frap_imbuf::FileFormat::detect(bytes).map(|f| f.extension())
}

/// `BLI_path_make_safe_filename`: characters unsafe in file names become `_`.
fn safe_filename(s: &str) -> String {
    s.chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect()
}

/// `unpack_generate_paths`: (absolute = original, local) paths of the packed file.
fn generate_paths(im: &Image, bytes: &[u8]) -> (String, String) {
    let fp = &im.packed_filepath;
    let (dir, file) = match fp.rfind(['/', '\\']) {
        Some(i) => (fp[..=i].to_owned(), fp[i + 1..].to_owned()),
        None => (String::new(), fp.clone()),
    };
    let file = if file.is_empty() {
        let mut f = im.header.name.to_string();
        if let Some(ext) = extension_of(bytes)
            && !f.to_ascii_lowercase().ends_with(ext)
        {
            f.push_str(ext);
        }
        safe_filename(&f)
    } else {
        file
    };
    let dir = if dir.is_empty() { "//".to_owned() } else { dir };
    (format!("{dir}{file}"), format!("//textures/{file}"))
}

/// `BKE_packedfile_write_to_file`
fn write_to_file(base: &PathBase, path: &str, bytes: &[u8], rep: &mut Reports) -> bool {
    let abs = base.abs(path);
    // An existing file is replaced (Blender backs it up while writing and removes the copy).
    match write_file(base, &abs, bytes) {
        Ok(()) => {
            rep.info(format!("Saved packed file to: {abs}"));
            true
        }
        Err(_) => {
            rep.error(format!("Error creating file '{abs}'"));
            false
        }
    }
}

/// `BKE_packedfile_unpack_image`; checks of `rna_Image_unpack` included.
pub(crate) fn unpack(
    h: &mut dyn Host,
    r: IdRef<ImageId>,
    method: Method,
    rep: &mut Reports,
) -> Result<(), RnaError> {
    let base = PathBase::of(h);
    let main = h.main();
    let im = main.images.get(r).ok_or_else(|| internal("the image was removed"))?;
    let Some(packed) = im.packed_file.clone() else {
        rep.error("Image not packed");
        return Ok(());
    };
    if im.header.library.is_some() {
        rep.error("Image is not editable");
        return Ok(());
    }
    if matches!(im.source.as_str(), "MOVIE" | "SEQUENCE") {
        rep.error("Unpacking movies or image sequences not supported");
        return Ok(());
    }
    let (abs_name, local_name) = generate_paths(im, &packed.bytes);
    let exists = |p: &str| base.file(p).exists();
    let new_path = match method {
        Method::Remove => Some(abs_name.clone()),
        Method::UseLocal if exists(&local_name) => Some(local_name.clone()),
        Method::UseLocal | Method::WriteLocal => {
            write_to_file(&base, &local_name, &packed.bytes, rep).then(|| local_name.clone())
        }
        Method::UseOriginal if exists(&abs_name) => {
            rep.info(format!("Use existing file (instead of packed): {abs_name}"));
            Some(abs_name.clone())
        }
        Method::UseOriginal | Method::WriteOriginal => {
            write_to_file(&base, &abs_name, &packed.bytes, rep).then(|| abs_name.clone())
        }
    };
    let (main, log) = h.main_mut();
    if let Some(im) = main.images.get_mut(r, log) {
        // The packed file is dropped whether the write worked or not.
        im.packed_file = None;
        im.packed_filepath.clear();
        if let Some(p) = new_path
            && method != Method::Remove
        {
            im.filepath = p;
        }
    }
    Ok(())
}

#[rna_func(srna = "Image", func = "unpack")]
fn image_unpack(h: &mut dyn Host, this: &RnaPtr, args: &mut FuncArgs) -> Result<Value, RnaError> {
    let r = image_ref(this)?;
    let method = Method::from_identifier(arg_str(args, "method"));
    let mut rep = Reports::default();
    unpack(h, r, method, &mut rep)?;
    rep.finish(h)?;
    Ok(Value::None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths() {
        // Verified on 5.2.4: `sub/u3.png` → original `sub/u3.png`, local `//textures/u3.png`;
        // a generated image packed from memory ('Gen Img', no path) → `//Gen Img.png`,
        // `//textures/Gen Img.png`.
        let png = [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];
        let mut im = Image::new("u3.png", 1, 1);
        im.packed_filepath = "sub/u3.png".into();
        assert_eq!(
            generate_paths(&im, &png),
            ("sub/u3.png".into(), "//textures/u3.png".into())
        );
        let im = Image::new("Gen Img", 1, 1);
        assert_eq!(
            generate_paths(&im, &png),
            ("//Gen Img.png".into(), "//textures/Gen Img.png".into())
        );
        assert_eq!(Method::from_identifier("WRITE_LOCAL"), Method::WriteLocal);
        assert_eq!(Method::from_identifier(""), Method::UseLocal);
    }
}
