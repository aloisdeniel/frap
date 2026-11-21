// SPDX-License-Identifier: GPL-3.0-or-later
//! Output path rules of `render.render` [OBJ §4.17]: `//` relative to the `.frap` or cwd,
//! relative paths against the cwd, `use_file_extension`, `#` runs zero-padded (WP 1.24).
//!
//! The order is Blender's `BKE_image_path_from_imformat` / `do_makepicstring`
//! (`blenkernel/intern/image_format.cc`, Blender 5.2.4, GPL-2.0-or-later), checked against
//! 5.2.4 for every rule below:
//!
//! 1. path templates (`{scene_name}`, …; [`template`](crate::output::template)): parse errors
//!    fail the render;
//! 2. `//` is made absolute against the directory of `bpy.data.filepath`; unsaved, `//` is
//!    dropped and the rest is relative to the cwd (`//out/a` → `out/a.png`); the path is then
//!    normalised (`BLI_path_normalize`: `out/../out/a_` → `out/a_`, `./a` → `a`, `a//b` → `a/b`);
//! 3. animation frames only (`BLI_path_frame`, 4 digits): the **last** run of `#` in the file
//!    name (not in directory names) becomes the frame, zero-padded to the run's length with the
//!    sign in front (`-002`); without a `#` the frame is appended with 4 digits (`anim_0003`,
//!    `dir/0003`, `n.png0003`). A still keeps its `#` (`hash_###_x.png`);
//! 4. a path ending in a directory separator (or `..`) gets the file name `Untitled`;
//! 5. with `use_file_extension`, the format's extension is ensured
//!    (`BKE_image_path_ext_from_imformat_ensure`): kept when the path already ends in one of
//!    the format's extensions (any case: `shot.PNG`, `shot.jpeg` and `shot.JPG` for JPEG),
//!    replacing a known image extension (`shot.jpg` → `shot.png`, `shot.tif` → `shot.png`,
//!    `shot.` and `shot..` → `shot.png`), appended otherwise (`shot.foo` → `shot.foo.png`,
//!    `shot.png.` → `shot.png.png`).
//!
//! Relative results are relative to the session cwd (frap's [`frap_op::PathResolver`]).
//! Missing parent directories are created, as Blender does. **Deviation:** Blender 5.2.4 fails
//! to save a path without a directory part (`filepath = 'shot'`: `Render error (No such file
//! or directory) cannot save: 'shot.png'`, `BLI_file_ensure_parent_dir_exists` on an empty
//! parent); frap writes it to the cwd.
//!
//! [`format`] maps `image_settings` to an encoder and converts the colour-managed film.

use std::path::{Path, PathBuf};

pub mod format;
pub mod template;

pub use template::{TemplateValue, TemplateVars};

/// Directory separators Blender's path functions accept on every platform.
fn is_sep(c: char) -> bool {
    c == '/' || c == '\\'
}

/// Byte offset of the file name (after the last separator).
fn file_name_start(path: &str) -> usize {
    path.rfind(is_sep).map_or(0, |i| i + 1)
}

/// `BLI_path_frame(path, frame, digits)`: replaces the last run of `#` in the file name by
/// `frame` (`%.*d`: zero-padded to the run's length, sign first), or appends `digits` digits
/// when the file name has no `#`.
pub fn frame_path(path: &str, frame: i32, digits: usize) -> String {
    let start = file_name_start(path);
    let name = &path[start..];
    // Last run of '#'.
    let run = name
        .char_indices()
        .rfind(|(_, c)| *c == '#')
        .map(|(end, _)| {
            let end = end + 1;
            let begin = name[..end].trim_end_matches('#').len();
            (start + begin, start + end)
        });
    let (path, (a, b)) = match run {
        Some(r) => (path.to_owned(), r),
        None => {
            let p = format!("{path}{}", "#".repeat(digits));
            let n = p.len();
            (p, (n - digits, n))
        }
    };
    let width = b - a;
    let digits = if frame < 0 {
        format!("-{:0width$}", -(frame as i64))
    } else {
        format!("{frame:0width$}")
    };
    format!("{}{digits}{}", &path[..a], &path[b..])
}

/// The file extensions a `file_format` writes (`image_format_ext_array`), first = default.
/// Empty for movie formats.
pub fn format_extensions(file_format: &str) -> &'static [&'static str] {
    match file_format {
        "BMP" => &[".bmp"],
        "IRIS" => &[".rgb"],
        "PNG" => &[".png"],
        "JPEG" => &[".jpg", ".jpeg"],
        "TARGA" | "TARGA_RAW" => &[".tga"],
        "JPEG2000" => &[".jp2", ".j2c"],
        "CINEON" => &[".cin"],
        "DPX" => &[".dpx"],
        "OPEN_EXR" | "OPEN_EXR_MULTILAYER" => &[".exr"],
        "HDR" => &[".hdr"],
        "TIFF" => &[".tif", ".tiff"],
        "WEBP" => &[".webp"],
        _ => &[],
    }
}

/// Extensions Blender knows as images (`imb_ext_image`); one of them is replaced rather than
/// kept when it is not the format's own (verified list: `.gif`, `.mp4`, `.heic`, `.ktx`,
/// `.svg`, … are appended to instead).
pub const KNOWN_IMAGE_EXTENSIONS: &[&str] = &[
    ".png", ".tga", ".bmp", ".jpg", ".jpeg", ".sgi", ".rgb", ".rgba", ".tif", ".tiff", ".tx",
    ".jp2", ".j2c", ".hdr", ".dds", ".dpx", ".cin", ".exr", ".psd", ".pdd", ".psb", ".webp",
    ".avif",
];

fn has_extension(path: &str, exts: &[&str]) -> bool {
    let lower = path.to_ascii_lowercase();
    exts.iter().any(|e| lower.ends_with(e))
}

/// `BKE_image_path_ext_from_imformat_ensure`.
pub fn ensure_extension(path: &str, file_format: &str) -> String {
    let exts = format_extensions(file_format);
    let Some(ext) = exts.first() else {
        return path.to_owned();
    };
    if has_extension(path, exts) {
        return path.to_owned();
    }
    let name_start = file_name_start(path);
    if has_extension(&path[name_start..], KNOWN_IMAGE_EXTENSIONS) {
        // BLI_path_extension_replace: from the last '.' of the file name.
        let dot = path
            .rfind('.')
            .expect("invariant: a known extension has a dot");
        return format!("{}{ext}", &path[..dot]);
    }
    // BLI_path_extension_ensure: trailing dots are dropped, then the extension is appended.
    let trimmed = path.trim_end_matches('.');
    let trimmed = if trimmed.len() < name_start {
        &path[..name_start]
    } else {
        trimmed
    };
    format!("{trimmed}{ext}")
}

/// What to name an output file.
#[derive(Clone, Debug, PartialEq)]
pub struct OutputRules<'a> {
    /// `render.filepath`, raw.
    pub filepath: &'a str,
    /// Template variables.
    pub vars: &'a TemplateVars,
    /// `bpy.data.filepath` (`None`: unsaved).
    pub blend_file: Option<&'a str>,
    /// `render.use_file_extension`.
    pub use_file_extension: bool,
    /// `image_settings.file_format`.
    pub file_format: &'a str,
}

/// A computed output path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputPath {
    /// The path as Blender computes and prints it (`Saved: 'out/shot.png'`): relative stays
    /// relative to the cwd, `//` is resolved.
    pub path: String,
    /// Where the file is written.
    pub absolute: PathBuf,
}

/// Computes the output path of a still (`frame: None`) or of one animation frame.
///
/// `Err` is the operator's error message (template parse errors).
pub fn output_path(
    rules: &OutputRules,
    cwd: &Path,
    frame: Option<i32>,
) -> Result<OutputPath, String> {
    let p = template::apply(rules.filepath, rules.vars)
        .map_err(|e| template::error_message(rules.filepath, &e))?;
    let p = match p.strip_prefix("//") {
        Some(rest) => match rules.blend_file {
            Some(blend) => {
                let blend = absolute(Path::new(blend), cwd);
                let dir = blend.parent().map(Path::to_path_buf).unwrap_or_default();
                join_str(&dir, rest)
            }
            None => rest.to_owned(),
        },
        None => p,
    };
    // `BLI_path_abs` ends with `BLI_path_normalize` (`out/../out/a_` → `out/a_`).
    let mut p = normalize_path(&p);
    if let Some(f) = frame {
        p = frame_path(&p, f, 4);
    }
    let name = &p[file_name_start(&p)..];
    if !p.is_empty() && (name.is_empty() || name == "..") {
        if name == ".." {
            p.push('/');
        }
        p.push_str("Untitled");
    }
    if rules.use_file_extension {
        p = ensure_extension(&p, rules.file_format);
    }
    let abs = absolute(Path::new(&p), cwd);
    Ok(OutputPath {
        path: p,
        absolute: abs,
    })
}

/// The separators `BLI_path_normalize` collapses: `SEP` (`/`), and `\\` too on Windows where
/// `BLI_path_abs` turns `/` into `\\` first.
fn is_native_sep(c: u8) -> bool {
    c == b'/' || (cfg!(windows) && c == b'\\')
}

/// `BLI_path_normalize`: `//` and `/./` become `/`, a leading `./` is dropped, `dir/..` is
/// collapsed (leading `..` of a relative path are kept, those of an absolute path dropped), an
/// empty relative result is `.`. A leading `//` (blend-relative) is left alone.
// Ported from blender/source/blender/blenlib/intern/path_utils.cc (`path_normalize_impl`,
// Blender 5.2.4, GPL-2.0-or-later).
pub fn normalize_path(path: &str) -> String {
    let mut b = path.as_bytes().to_vec();
    let mut prefix = 0usize;
    if b.len() >= 2 && b[0] == b'/' && b[1] == b'/' {
        prefix = 2;
        let n = b[2..].iter().take_while(|&&c| is_native_sep(c)).count();
        b.drain(2..2 + n);
    }
    if cfg!(windows) && prefix == 0 && b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
        prefix = 2;
    }
    let mut s = b.split_off(prefix);
    normalize_tail(&mut s, prefix == 0);
    b.extend_from_slice(&s);
    // Only ASCII bytes were removed, at ASCII boundaries.
    String::from_utf8(b).unwrap_or_else(|_| path.to_owned())
}

/// `path_normalize_impl` after the prefix (`//`, a drive): `at_start` when nothing was skipped.
fn normalize_tail(s: &mut Vec<u8>, at_start: bool) {
    let sep = is_native_sep;
    let is_relative = s.first().is_some_and(|&c| !sep(c));
    // Inline replacement: `/./` → `/`, `//` → `/`.
    if s.len() > 1 {
        let mut i = s.len() - 1;
        while i > 0 {
            if sep(s[i]) {
                let i_end = i;
                while i > 0 {
                    if sep(s[i - 1]) {
                        i -= 1;
                    } else if i >= 2 && s[i - 1] == b'.' && sep(s[i - 2]) {
                        i -= 2;
                    } else {
                        break;
                    }
                }
                if i < i_end {
                    s.drain(i..i_end);
                }
            }
            if i == 0 {
                break;
            }
            i -= 1;
        }
    }
    // A redundant leading `./`.
    if is_relative && s.len() > 2 && s[0] == b'.' && sep(s[1]) {
        s.drain(0..2);
    }
    // Collapse parent directories (`<parent>/<child>/../` → `<parent>/`).
    let at = |s: &[u8], k: usize| s.get(k).copied().unwrap_or(0);
    let is_parent = |s: &[u8], k: usize| {
        at(s, k) == b'.' && at(s, k + 1) == b'.' && (sep(at(s, k + 2)) || at(s, k + 2) == 0)
    };
    let first_non_slash = s.iter().take_while(|&&c| sep(c)).count();
    let mut start_base = first_non_slash;
    while is_parent(s, start_base) {
        start_base += 3;
    }
    if start_base < s.len() {
        let mut start = start_base;
        loop {
            let len = s.len();
            let found = (start..len.saturating_sub(3))
                .find(|&k| sep(s[k]) && s[k + 1] == b'.' && s[k + 2] == b'.' && sep(s[k + 3]))
                .or_else(|| {
                    // The path ends with `/..`.
                    let tail = len.checked_sub(3)?;
                    (start <= tail && sep(s[tail]) && s[tail + 1] == b'.' && s[tail + 2] == b'.')
                        .then_some(tail)
                });
            let Some(found) = found else { break };
            start = found + 1;
            let mut end_all = start;
            loop {
                end_all += 3;
                if !is_parent(s, end_all) {
                    break;
                }
            }
            let mut end = start;
            loop {
                loop {
                    start -= 1;
                    if !(start_base < start && !sep(s[start - 1])) {
                        break;
                    }
                }
                end += 3;
                if !(start != start_base && end < end_all) {
                    break;
                }
            }
            if end > len {
                end -= 1;
                end_all -= 1;
            }
            s.drain(start..end);
            if start == start_base && end != end_all {
                start_base += end_all - end;
                start = if start_base < s.len() {
                    start_base
                } else {
                    start_base - 1
                };
            }
        }
    }
    if is_relative {
        if s.is_empty() && at_start {
            s.push(b'.');
        }
    } else if start_base != first_non_slash {
        // `/../home/me` → `/home/me`.
        let st = if start_base > s.len() {
            start_base - 1
        } else {
            start_base
        };
        s.drain(first_non_slash..st);
    }
}

fn join_str(dir: &Path, rest: &str) -> String {
    let d = dir.to_string_lossy();
    if d.is_empty() {
        rest.to_owned()
    } else if d.ends_with(is_sep) {
        format!("{d}{rest}")
    } else {
        format!("{d}{}{rest}", std::path::MAIN_SEPARATOR)
    }
}

fn absolute(p: &Path, cwd: &Path) -> PathBuf {
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        cwd.join(p)
    }
}

/// Writes `bytes` to `path`, creating missing parent directories (as Blender does), through a
/// temporary sibling renamed into place so a failed write never leaves a partial file. OS
/// errors read as C `strerror` (Blender's `strerror(errno)`), on Windows too.
pub fn write_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use frap_rna::error::strerror;
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        // Blender only logs `Couldn't create directory for file …`; the save that follows fails
        // and reports this, with the `errno` the directory left (`Read-only file system`).
        std::fs::create_dir_all(dir).map_err(|e| {
            format!(
                "Render error ({}) cannot save: '{}'",
                strerror(&e),
                path.display()
            )
        })?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".frap-tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, bytes).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!(
            "Render error ({}) cannot save: '{}'",
            strerror(&e),
            path.display()
        )
    })?;
    // `--stmt-timeout` (§12): a statement past its deadline answers `TIMEOUT` without leaving
    // the file (the temporary one is removed before unwinding).
    if frap_math::budget::expired() {
        let _ = std::fs::remove_file(&tmp);
        frap_math::budget::check_now();
    }
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!(
            "Render error ({}) cannot save: '{}'",
            strerror(&e),
            path.display()
        )
    })
}

/// [`write_file`] for an OpenEXR render result (`BKE_image_render_write_exr`): when the folder
/// cannot be made, the OpenEXR writer still opens the file and fails with that `errno`
/// (`No such file or directory`), reported twice as Blender does (verified on 5.2.4 for
/// `filepath='/nonexistent_dir/x/y'`: `Error writing render result, No such file or directory
/// (see console)`, then `Render error (No such file or directory) cannot save: '…'`).
pub fn write_exr_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use frap_rna::error::strerror;
    let dir_failed = path
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .is_some_and(|d| std::fs::create_dir_all(d).is_err());
    let open = if dir_failed {
        std::fs::File::create(path).map(|_| ())
    } else {
        write_file(path, bytes).map_err(std::io::Error::other)
    };
    match open {
        Ok(()) if dir_failed => {
            // The folder appeared meanwhile: write normally.
            write_file(path, bytes)
        }
        Ok(()) => Ok(()),
        Err(e) if dir_failed => {
            let e = strerror(&e);
            Err(format!(
                "Error writing render result, {e} (see console)\nError: Render error ({e}) \
                 cannot save: '{}'",
                path.display()
            ))
        }
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exr_write_error_reports_the_open_errno_twice() {
        // A folder that cannot be created (a regular file stands in its way).
        let dir = std::env::temp_dir().join(format!("frap_exr_err_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let blocker = dir.join("blocker");
        std::fs::write(&blocker, b"x").unwrap();
        let path = blocker.join("x").join("y.exr");
        let e = write_exr_file(&path, b"exr").unwrap_err();
        let first = e.lines().next().unwrap();
        assert!(first.starts_with("Error writing render result, "), "{e}");
        assert!(first.ends_with(" (see console)"), "{e}");
        assert!(e.contains("\nError: Render error ("), "{e}");
        assert!(
            e.ends_with(&format!("cannot save: '{}'", path.display())),
            "{e}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn vars() -> TemplateVars {
        TemplateVars::render(None, "Scene", Some("Camera"), [16, 8], 24, 1.0)
    }

    fn still(fp: &str) -> String {
        still_fmt(fp, "PNG", true)
    }

    fn still_fmt(fp: &str, fmt: &str, ext: bool) -> String {
        let v = vars();
        let r = OutputRules {
            filepath: fp,
            vars: &v,
            blend_file: None,
            use_file_extension: ext,
            file_format: fmt,
        };
        output_path(&r, Path::new("/cwd"), None).unwrap().path
    }

    fn anim(fp: &str, frame: i32) -> String {
        let v = vars();
        let r = OutputRules {
            filepath: fp,
            vars: &v,
            blend_file: None,
            use_file_extension: true,
            file_format: "PNG",
        };
        output_path(&r, Path::new("/cwd"), Some(frame))
            .unwrap()
            .path
    }

    /// `BLI_path_normalize` (Blender 5.2.4's `path_normalize_impl`, compiled on its own and run
    /// on ~74k generated paths, agrees with this port everywhere it does not read out of
    /// bounds; these are a sample).
    #[test]
    fn normalize_like_blender() {
        for (p, n) in [
            ("", ""),
            (".", "."),
            ("..", ".."),
            ("a/..", "."),
            ("/..", "/"),
            ("/../home/me", "/home/me"),
            ("out/../out/r_", "out/r_"),
            ("./out/./n_", "out/n_"),
            ("out//dd_", "out/dd_"),
            ("out/sub/..", "out/"),
            ("out/sub/../", "out/"),
            ("../up/out/x_", "../up/out/x_"),
            ("a/../../b", "../b"),
            ("../../x/../y", "../../y"),
            ("out/a/../../out/b/../c_", "out/c_"),
            ("//a/../b", "//b"),
            ("//./x", "//x"),
            ("./", "./"),
            ("a/./", "a/"),
            ("/a//b/./c/", "/a/b/c/"),
        ] {
            assert_eq!(normalize_path(p), n, "{p:?}");
        }
    }

    /// Observed on Blender 5.2.4 (`Saved: '…'`).
    #[test]
    fn paths_are_normalised() {
        assert_eq!(anim("out/../out/r_", 1), "out/r_0001.png");
        assert_eq!(anim("./out/./n_", 1), "out/n_0001.png");
        assert_eq!(anim("out//dd_", 1), "out/dd_0001.png");
        assert_eq!(anim("out/a/../../out/b/../c_", 1), "out/c_0001.png");
        assert_eq!(anim("//out/q/../g_", 1), "out/g_0001.png");
        assert_eq!(anim("../wp/out/up_", 1), "../wp/out/up_0001.png");
        assert_eq!(anim("out/sub/..", 1), "out/0001.png");
        assert_eq!(still("out/sub/../s2"), "out/s2.png");
        assert_eq!(still("out/sub/.."), "out/Untitled.png");
        assert_eq!(still("out/sub/../"), "out/Untitled.png");
    }

    // Every expectation below was observed on Blender 5.2.4 (file names written by
    // `render.render(write_still=True)` / `render.render(animation=True)`).
    #[test]
    fn still_paths() {
        assert_eq!(still("out/shot"), "out/shot.png");
        assert_eq!(still("out/hash_###_x"), "out/hash_###_x.png");
        assert_eq!(still("out/shot.png"), "out/shot.png");
        assert_eq!(still("out/shot.PNG"), "out/shot.PNG");
        assert_eq!(still("out/shot.Png"), "out/shot.Png");
        for e in [
            "jpg", "JPG", "exr", "tif", "TIFF", "jpeg", "tga", "hdr", "webp", "bmp", "j2c", "dpx",
            "cin", "rgb", "sgi", "rgba", "tx", "jp2", "dds", "psd", "avif", "pdd", "psb",
        ] {
            assert_eq!(still(&format!("o/shot.{e}")), "o/shot.png", "{e}");
        }
        for e in [
            "foo", "mp4", "gif", "heic", "ktx", "ktx2", "svg", "ico", "pic", "jxl", "mov", "tex",
        ] {
            assert_eq!(
                still(&format!("o/shot.{e}")),
                format!("o/shot.{e}.png"),
                "{e}"
            );
        }
        assert_eq!(still("o/shot.png.png"), "o/shot.png.png");
        assert_eq!(still("o/shot."), "o/shot.png");
        assert_eq!(still("o/shot.."), "o/shot.png");
        assert_eq!(still("o/shot.png."), "o/shot.png.png");
        assert_eq!(still("o/a.b/c"), "o/a.b/c.png");
        assert_eq!(still("o/a.b.c"), "o/a.b.c.png");
        assert_eq!(still("o/dir##/x"), "o/dir##/x.png");
        assert_eq!(still("o/sub/"), "o/sub/Untitled.png");
        assert_eq!(still("o/sub/."), "o/sub/.png");
        assert_eq!(still("o/.png"), "o/.png");
        assert_eq!(still("o/a#"), "o/a#.png");
        assert_eq!(
            still("o/res_{resolution_x}x{resolution_y}"),
            "o/res_16x8.png"
        );
        assert_eq!(still("o/brace_{{x}}"), "o/brace_{x}.png");
        assert_eq!(still(""), ".png");
        assert_eq!(still("//"), ".png");
        assert_eq!(still("//out/rel"), "out/rel.png");
        assert_eq!(still_fmt("o/noext", "PNG", false), "o/noext");
        assert_eq!(still_fmt("o/noext2.jpg", "PNG", false), "o/noext2.jpg");
        assert_eq!(still_fmt("o/sub/", "PNG", false), "o/sub/Untitled");
    }

    #[test]
    fn extension_per_format() {
        assert_eq!(still_fmt("o/j", "JPEG", true), "o/j.jpg");
        assert_eq!(still_fmt("o/e", "OPEN_EXR", true), "o/e.exr");
        assert_eq!(still_fmt("o/h", "HDR", true), "o/h.hdr");
        assert_eq!(still_fmt("o/t", "TARGA", true), "o/t.tga");
        assert_eq!(still_fmt("o/tr", "TARGA_RAW", true), "o/tr.tga");
        assert_eq!(still_fmt("o/b", "BMP", true), "o/b.bmp");
        assert_eq!(still_fmt("o/w", "WEBP", true), "o/w.webp");
        assert_eq!(still_fmt("o/shot.png", "JPEG", true), "o/shot.jpg");
        assert_eq!(still_fmt("o/shot.jpeg", "JPEG", true), "o/shot.jpeg");
        assert_eq!(still_fmt("o/shot.JPG", "JPEG", true), "o/shot.JPG");
        assert_eq!(still_fmt("o/shot.png", "OPEN_EXR", true), "o/shot.exr");
        assert_eq!(still_fmt("o/shot.tga", "TARGA", true), "o/shot.tga");
        assert_eq!(still_fmt("o/shot.png", "TARGA", true), "o/shot.tga");
    }

    #[test]
    fn animation_paths() {
        assert_eq!(anim("o/anim_", 3), "o/anim_0003.png");
        assert_eq!(anim("o/anim2_##_z", 7), "o/anim2_07_z.png");
        assert_eq!(anim("o/dir/", 3), "o/dir/0003.png");
        assert_eq!(anim("o/x_#_y_##", 3), "o/x_#_y_03.png");
        assert_eq!(anim("o/x_#####", 4), "o/x_00004.png");
        assert_eq!(anim("o/n", 3), "o/n0003.png");
        assert_eq!(anim("o/n.png", 3), "o/n.png0003.png");
        assert_eq!(anim("o/n.png#", 3), "o/n.png3.png");
        assert_eq!(anim("o/neg_", -1), "o/neg_-0001.png");
        assert_eq!(anim("o/neg_###", -2), "o/neg_-002.png");
        assert_eq!(anim("o/big_", 12345), "o/big_12345.png");
        assert_eq!(anim("o/big_##", 12345), "o/big_12345.png");
        assert_eq!(anim("o/d##/f", 2), "o/d##/f0002.png");
        assert_eq!(anim("o/f{resolution_x}#", 2), "o/f162.png");
        assert_eq!(anim("o/{{#}}", 2), "o/{2}.png");
        assert_eq!(anim("", 2), "0002.png");
    }

    #[test]
    fn frame_path_rules() {
        assert_eq!(frame_path("a/b#/c", 5, 4), "a/b#/c0005");
        assert_eq!(frame_path("a\\b#\\c##", 5, 4), "a\\b#\\c05");
        assert_eq!(frame_path("c", i32::MIN, 4), format!("c{}", i32::MIN));
    }

    #[test]
    fn relative_to_saved_file_and_cwd() {
        let v = vars();
        let rules = |fp| OutputRules {
            filepath: fp,
            vars: &v,
            blend_file: Some("/proj/saved/my file.frap"),
            use_file_extension: true,
            file_format: "PNG",
        };
        let sep = std::path::MAIN_SEPARATOR;
        let o = output_path(&rules("//out/rel"), Path::new("/cwd"), None).unwrap();
        assert_eq!(o.path, format!("/proj/saved{sep}out/rel.png"));
        assert_eq!(o.absolute, Path::new("/proj/saved").join("out/rel.png"));
        let o = output_path(&rules("//"), Path::new("/cwd"), None).unwrap();
        assert_eq!(o.path, format!("/proj/saved{sep}Untitled.png"));
        let o = output_path(&rules("out/x"), Path::new("/cwd"), None).unwrap();
        assert_eq!(o.path, "out/x.png");
        assert_eq!(o.absolute, Path::new("/cwd").join("out/x.png"));
        // A relative `.frap` path is relative to the cwd.
        let v2 = vars();
        let r = OutputRules {
            filepath: "//a",
            vars: &v2,
            blend_file: Some("p/s.frap"),
            use_file_extension: true,
            file_format: "PNG",
        };
        let o = output_path(&r, Path::new("/cwd"), None).unwrap();
        assert_eq!(o.absolute, Path::new("/cwd/p").join("a.png"));
    }

    #[test]
    fn template_errors_fail() {
        let v = vars();
        let r = OutputRules {
            filepath: "out/{nope}",
            vars: &v,
            blend_file: None,
            use_file_extension: true,
            file_format: "PNG",
        };
        assert_eq!(
            output_path(&r, Path::new("/cwd"), None).unwrap_err(),
            "Parse errors in path 'out/{nope}':\n\
             - Unknown variable referenced in template expression '{nope}'."
        );
    }

    #[test]
    fn writes_with_parent_dirs() {
        let dir = std::env::temp_dir().join(format!("frap-ops-render-out-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let p = dir.join("a/b/c.bin");
        write_file(&p, b"xyz").unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"xyz");
        assert!(!dir.join("a/b/c.bin.frap-tmp").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
