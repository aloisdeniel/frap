// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.image.open(filepath='', directory='', files=[], relative_path=True,
//! use_sequence_detection=True, use_udim_detecting=True, …)`. Port of `image_open_exec` /
//! `image_open_single` (`editors/space_image/image_ops.cc`) and
//! `ED_image_filesel_detect_sequences` (`image_sequence.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! - With `directory` and `files` set, every `directory/<name>` is opened; with
//!   `use_sequence_detection`, consecutive names that differ only by their frame number
//!   (`img_001.png`, `img_002.png`) form one `SEQUENCE` image whose path is the lowest frame.
//!   The directory is made `//`-relative with `relative_path` when the `.frap` file is saved.
//!   Otherwise `filepath` is opened as given.
//! - An image already loaded from the same path is reused (`BKE_image_load_exists`), its path
//!   updated to the one given; a new image is a FILE image named after the file, with no users
//!   (verified on 5.2.4), exactly as `bpy.data.images.load`.
//! - A path that cannot be opened reports `Cannot read '<path>': No such file or directory`
//!   (verified on 5.2.4: `RuntimeError: Error: Cannot read '/nonexist/x.png': No such file or
//!   directory`).
//!
//! UDIM detection (`<UDIM>` tiles) is not supported: such files open as single images.

use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{IdRef, ImageId};
use frap_scene::Main;
use frap_scene::id::{Image, LoadError, PathBase};
use frap_spec_ops_data::image;
use smol_str::SmolStr;

/// `files[*].name`
fn file_names(v: &Value) -> Vec<String> {
    match v {
        Value::Structs(items) => items
            .iter()
            .filter_map(|m| m.get("name").and_then(Value::as_str).map(str::to_owned))
            .filter(|n| !n.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

/// `BLI_path_sequence_decode`: (head, frame, tail) of a file name whose last digit run comes
/// right before the extension (`img_012.png` → (`img_`, 12, `.png`)); frame 0 and the whole
/// name as head without one.
fn sequence_decode(name: &str) -> (String, i64, String) {
    let ext_at = name.rfind('.').filter(|&i| i > 0).unwrap_or(name.len());
    let stem = &name[..ext_at];
    let digits = stem.chars().rev().take_while(char::is_ascii_digit).count();
    if digits == 0 {
        return (name.to_owned(), 0, String::new());
    }
    let start = stem.len() - digits;
    let frame = stem[start..].parse().unwrap_or(0);
    (stem[..start].to_owned(), frame, name[ext_at..].to_owned())
}

/// `BLI_path_join` of a directory and a file name.
fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() || dir.ends_with('/') || dir.ends_with('\\') {
        format!("{dir}{name}")
    } else {
        format!("{dir}/{name}")
    }
}

/// `BLI_path_rel`: `path` relative to the folder of `blend` (`//…`, with `..` steps), or
/// unchanged when it is not absolute or there is no saved file.
fn path_rel(path: &str, blend: Option<&str>) -> String {
    use std::path::{Component, Path};
    // A `/`-rooted path counts as absolute on Windows too (paths are platform-neutral).
    let absolute = path.starts_with('/') || Path::new(path).is_absolute();
    let (Some(blend), true) = (blend, absolute) else {
        return path.to_owned();
    };
    let Some(base) = Path::new(blend).parent() else {
        return path.to_owned();
    };
    let a: Vec<Component> = Path::new(path).components().collect();
    let b: Vec<Component> = base.components().collect();
    let common = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    if common == 0 {
        return path.to_owned();
    }
    let mut out = String::from("//");
    for _ in common..b.len() {
        out.push_str("../");
    }
    let rest: Vec<String> = a[common..]
        .iter()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    out.push_str(&rest.join("/"));
    if path.ends_with('/') && !out.ends_with('/') {
        out.push('/');
    }
    out
}

/// One file (or image sequence) to open.
struct Range {
    filepath: String,
    frames: Vec<i64>,
}

/// `ED_image_filesel_detect_sequences`
fn ranges(p: &image::Open, blend: Option<&str>) -> Vec<Range> {
    let names = file_names(&p.files.value);
    if p.directory.is_set && p.files.is_set && !names.is_empty() {
        let mut dir = p.directory.value.to_string();
        if let Some(rest) = dir.strip_prefix("//") {
            dir = PathBase {
                blend: blend.map(str::to_owned),
                cwd: None,
            }
            .abs(&format!("//{rest}"));
        }
        if p.relative_path.value {
            dir = path_rel(&dir, blend);
        }
        let mut out: Vec<Range> = Vec::new();
        let mut base: Option<(String, String)> = None;
        let mut first_frame = 0;
        for n in &names {
            let (head, frame, tail) = sequence_decode(n);
            let same = p.use_sequence_detection.value
                && !out.is_empty()
                && base.as_ref() == Some(&(head.clone(), tail.clone()));
            if same {
                let r = out.last_mut().expect("invariant: not empty");
                if frame < first_frame {
                    r.filepath = join(&dir, n);
                    first_frame = frame;
                }
                r.frames.push(frame);
            } else {
                out.push(Range {
                    filepath: join(&dir, n),
                    frames: vec![frame],
                });
                base = Some((head, tail));
                first_frame = frame;
            }
        }
        return out;
    }
    let fp = p.filepath.value.to_string();
    let file = fp.rsplit(['/', '\\']).next().unwrap_or("");
    if file.is_empty() {
        return Vec::new();
    }
    let (_, frame, _) = sequence_decode(file);
    vec![Range {
        filepath: fp,
        frames: vec![frame],
    }]
}

/// `image_detect_frame_range`: the number of consecutive frames from the lowest.
fn sequence_length(frames: &[i64]) -> i64 {
    let mut f = frames.to_vec();
    f.sort_unstable();
    let Some(&first) = f.first() else {
        return 1;
    };
    let mut cur = first;
    for &x in &f {
        if x == cur {
            cur += 1;
        }
    }
    cur - first
}

/// `BKE_image_load_exists`: an image (not GENERATED/VIEWER, not linked) with the same path
/// after `//` expansion.
fn existing(main: &Main, filepath: &str) -> Option<IdRef<ImageId>> {
    let base = PathBase {
        blend: main.filepath.clone(),
        cwd: None,
    };
    let want = base.abs(filepath);
    main.images
        .iter()
        .find(|(_, im)| {
            !matches!(im.source.as_str(), "VIEWER" | "GENERATED")
                && im.header.library.is_none()
                && base.abs(&im.filepath) == want
        })
        .map(|(r, _)| r)
}

#[operator(
    poll = poll::ALWAYS,
    tier = T0,
    spec = image::OPSPEC_IMAGE_OT_OPEN,
    notes = [
        "the image is shown nowhere (no Image Editor headless): assign it with node.image = bpy.data.images['name.png']",
        "with files=[…], a file that cannot be read raises and the whole statement is rolled back (frap statements are atomic); Blender reports an ERROR and keeps the files it could open",
        "UDIM tiles are not detected: such files open as single images",
    ]
)]
fn exec(cx: &mut OpCtx, p: image::Open) -> OpResult {
    let blend = cx.main().filepath.clone();
    let base = PathBase {
        blend: blend.clone(),
        cwd: Some(cx.services().paths().cwd.clone()),
    };
    let mut first = None;
    let mut errors = Vec::new();
    for range in ranges(&p, blend.as_deref()) {
        let (main, log) = cx.main_mut();
        if let Some(r) = existing(main, &range.filepath) {
            if let Some(im) = main.images.get_mut(r, log) {
                im.filepath = range.filepath.clone();
            }
            first.get_or_insert(r);
            continue;
        }
        match Image::load(&range.filepath, &base) {
            Ok(mut im) => {
                if im.source == "FILE" && sequence_length(&range.frames) > 1 {
                    im.source = SmolStr::new_static("SEQUENCE");
                }
                let name = im.header.name.clone();
                let r = main.images.insert(&name, im, log);
                first.get_or_insert(r);
            }
            Err(LoadError::Io(why)) => {
                errors.push(format!("Cannot read '{}': {why}", range.filepath));
            }
        }
    }
    if let Some(e) = errors.into_iter().next() {
        return Err(OpError::Runtime(e));
    }
    match first {
        Some(r) => {
            cx.result("image", r.any());
            Ok(OpStatus::Finished)
        }
        None => Ok(OpStatus::Cancelled),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequence_names() {
        assert_eq!(
            sequence_decode("img_012.png"),
            ("img_".into(), 12, ".png".into())
        );
        assert_eq!(
            sequence_decode("rgb.png"),
            ("rgb.png".into(), 0, String::new())
        );
        assert_eq!(sequence_length(&[3, 1, 2, 5]), 3);
        assert_eq!(sequence_length(&[7]), 1);
    }

    #[test]
    fn relative_paths() {
        assert_eq!(
            path_rel("/a/b/tex/x.png", Some("/a/b/scene.frap")),
            "//tex/x.png"
        );
        assert_eq!(
            path_rel("/a/c/x.png", Some("/a/b/scene.frap")),
            "//../c/x.png"
        );
        assert_eq!(path_rel("rel/x.png", Some("/a/b/scene.frap")), "rel/x.png");
        assert_eq!(path_rel("/a/x.png", None), "/a/x.png");
        assert_eq!(join("dir", "x.png"), "dir/x.png");
        assert_eq!(join("dir/", "x.png"), "dir/x.png");
    }
}
