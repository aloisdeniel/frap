// SPDX-License-Identifier: GPL-3.0-or-later
//! `gltf_roundtrip` (§17.2, §20.4): the differential round-trip law. For every file of the
//! allow-listed fixture set (`tests/data/gltf-roundtrip/allow.toml`),
//! `gltf_canonical(frap import → frap export) == gltf_canonical(Blender import → Blender
//! export)` with default options. GLB → GLB is **not** expected to equal the source.
//!
//! Allow-list format (owned by WP 1.23):
//!
//! ```toml
//! [[file]]
//! path = "khronos/Box.glb"      # relative to tests/data/
//! note = "why it is on the list" # optional
//! ```
//!
//! The Blender side is produced by [`blender_case`] through the golden runner
//! (`crate::blender::roundtrip`).

use std::path::Path;

use serde::Deserialize;

use super::gltf_canonical;
use crate::case::Mismatch;

/// Comparator name.
pub const NAME: &str = "gltf_roundtrip";

/// One allow-listed file.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct AllowEntry {
    /// Path relative to `tests/data/`.
    pub path: String,
    /// Why it is listed (or why it was hard).
    #[serde(default)]
    pub note: Option<String>,
}

/// The allow list.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct AllowList {
    /// Entries.
    #[serde(default, rename = "file")]
    pub files: Vec<AllowEntry>,
}

impl AllowList {
    /// Parses the TOML text.
    pub fn parse(text: &str) -> Result<AllowList, String> {
        let list: AllowList = toml::from_str(text).map_err(|e| format!("allow list: {e}"))?;
        for (i, f) in list.files.iter().enumerate() {
            if f.path.is_empty() || f.path.starts_with('/') || f.path.contains("..") {
                return Err(format!("allow list: entry {i}: bad path {:?}", f.path));
            }
            if list.files[..i].iter().any(|g| g.path == f.path) {
                return Err(format!("allow list: duplicate path {:?}", f.path));
            }
        }
        Ok(list)
    }
    /// Reads and parses a file; a missing file is an empty list.
    pub fn load(path: &Path) -> Result<AllowList, String> {
        match std::fs::read_to_string(path) {
            Ok(t) => AllowList::parse(&t),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(AllowList::default()),
            Err(e) => Err(format!("cannot read {}: {e}", path.display())),
        }
    }
    /// True when `path` (relative to `tests/data/`, `/`-separated) is listed.
    pub fn contains(&self, path: &str) -> bool {
        self.files.iter().any(|f| f.path == path)
    }
}

/// Checks the law for `source` given both tools' re-exported bytes (GLB or self-contained
/// glTF). `base_*` resolve external buffers of `.gltf` outputs.
pub fn check(
    source: &str,
    allow: &AllowList,
    frap_out: &[u8],
    base_frap: Option<&Path>,
    blender_out: &[u8],
    base_blender: Option<&Path>,
    tol: f32,
) -> Vec<Mismatch> {
    let what = format!("roundtrip({source})");
    if !allow.contains(source) {
        return vec![Mismatch::new(
            NAME,
            what,
            "not on the round-trip allow list (tests/data/gltf-roundtrip/allow.toml)",
        )];
    }
    let canon = |b: &[u8], base: Option<&Path>| {
        gltf_canonical::load(b, base).and_then(|d| gltf_canonical::canonicalize(&d))
    };
    match (canon(frap_out, base_frap), canon(blender_out, base_blender)) {
        (Ok(f), Ok(b)) => {
            let mut m = gltf_canonical::compare(&what, &f, &b, tol);
            for x in &mut m {
                x.comparator = NAME;
            }
            m
        }
        (Err(e), _) => vec![Mismatch::new(NAME, what, format!("frap output: {e}"))],
        (_, Err(e)) => vec![Mismatch::new(NAME, what, format!("blender output: {e}"))],
    }
}

fn py_str(s: &str) -> String {
    format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"))
}

/// The golden case that makes Blender (or frap) import `src` into an empty scene and export
/// it to `out` with default options.
pub fn blender_case(src: &Path, out: &str) -> String {
    format!(
        "# golden: startup=empty compare=glb:{out}\n\
         import bpy\n\
         bpy.ops.import_scene.gltf(filepath={})\n\
         bpy.ops.export_scene.gltf(filepath={})\n",
        py_str(&src.to_string_lossy()),
        py_str(out)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compare::gltf_canonical::tests::quad_scene;

    fn allow() -> AllowList {
        AllowList::parse(
            "[[file]]\npath = \"gltf-samples/Quad.glb\"\nnote = \"seed\"\n\
             [[file]]\npath = \"gltf-samples/Other.glb\"\n",
        )
        .unwrap()
    }

    #[test]
    fn parses_and_validates_the_allow_list() {
        let a = allow();
        assert!(a.contains("gltf-samples/Quad.glb"));
        assert!(!a.contains("Quad.glb"));
        assert_eq!(a.files[0].note.as_deref(), Some("seed"));
        assert!(AllowList::parse("[[file]]\npath = \"../x.glb\"\n").is_err());
        assert!(AllowList::parse("[[file]]\npath = \"a\"\n[[file]]\npath = \"a\"\n").is_err());
        assert_eq!(AllowList::parse("").unwrap(), AllowList::default());
        assert_eq!(
            AllowList::load(Path::new("/nonexistent/allow.toml")).unwrap(),
            AllowList::default()
        );
    }

    #[test]
    fn law_holds_under_permutation() {
        let f = quad_scene([0, 1, 2, 3], [[0, 1, 2], [0, 2, 3]], 0.0, "frap");
        let b = quad_scene([2, 3, 0, 1], [[1, 2, 0], [0, 2, 3]], 0.0, "Blender");
        assert_eq!(
            check("gltf-samples/Quad.glb", &allow(), &f, None, &b, None, 1e-5),
            vec![]
        );
    }

    #[test]
    fn law_fails_under_one_change_or_off_list() {
        let f = quad_scene([0, 1, 2, 3], [[0, 1, 2], [0, 2, 3]], 0.0, "frap");
        let g = quad_scene([0, 1, 2, 3], [[0, 1, 2], [0, 2, 3]], 0.25, "frap");
        let m = check("gltf-samples/Quad.glb", &allow(), &f, None, &g, None, 1e-5);
        assert!(!m.is_empty());
        assert!(m.iter().all(|x| x.comparator == NAME));
        let m = check("gltf-samples/Nope.glb", &allow(), &f, None, &f, None, 1e-5);
        assert_eq!(m.len(), 1);
        let m = check(
            "gltf-samples/Quad.glb",
            &allow(),
            b"junk",
            None,
            &f,
            None,
            1e-5,
        );
        assert!(m[0].detail.starts_with("frap output"));
    }

    #[test]
    fn blender_case_is_a_valid_case() {
        let text = blender_case(Path::new("/data/it's.glb"), "out/rt.glb");
        let c = crate::case::Case::parse(Path::new("rt.py"), &text).unwrap();
        assert_eq!(c.startup, crate::case::Startup::Empty);
        assert_eq!(
            c.compare,
            vec![crate::case::CompareSpec::Glb("out/rt.glb".into())]
        );
        assert!(text.contains("filepath='/data/it\\'s.glb'"));
    }
}
