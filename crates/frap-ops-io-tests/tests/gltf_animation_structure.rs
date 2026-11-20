// SPDX-License-Identifier: GPL-3.0-or-later
//! WP 6.7: the structure of frap's glTF animations against the blessed Blender GLBs of the
//! `tests/golden/cases/gltf/animation/` goldens.
//!
//! The golden `glb:` comparator (`gltf_canonical`) resamples every channel on the union of
//! the key times, so it does not see the sampler interpolation, the key count after
//! optimisation or the channel order. This test runs each case through frap (as the golden
//! runner does) and checks, per animation in file order, every channel's target node and
//! path, interpolation, exact key times and output count, and the channel order.
//!
//! Channel order: the add-on returns the F-curve channels in the order of the slot's
//! F-curves, then the channels it had to sample from `list(set(to_be_sampled))`, whose order
//! follows Python's randomised string hashing (it changes between Blender runs). For the
//! outputs listed in [`UNORDERED_TAIL`] the channels after the F-curve ones are compared as a
//! set.

use std::collections::BTreeMap;
use std::path::Path;

use frap_gltf::accessor::read_accessor;
use frap_gltf::json::animation::Interpolation;
use frap_golden::blender::{Repo, ScratchDir, fixture_file_name};
use frap_golden::case::{Case, CompareSpec};
use frap_golden::runner::{Props, RunOptions, run_frap};

/// `(animation, number of F-curve channels)`.
type Heads = &'static [(&'static str, usize)];

/// `(case, output, F-curve channels per animation)`: outputs whose sampled tail is in
/// Blender's hash order.
const UNORDERED_TAIL: &[(&str, &str, Heads)] = &[(
    "actions_sampling",
    "out/curves.glb",
    // Cube: location from the curves, then rotation and scale sampled.
    &[("CubeAction", 1)],
)];

/// One channel: target node name, path, interpolation, key times, output element count.
#[derive(Clone, Debug, PartialEq)]
struct Chan {
    node: String,
    path: String,
    interpolation: Interpolation,
    times: Vec<f32>,
    outputs: u64,
}

impl Chan {
    fn label(&self) -> String {
        format!("{}.{}", self.node, self.path)
    }
}

/// Every animation of a GLB/glTF file: name → channels in file order.
fn animations(bytes: &[u8]) -> Vec<(String, Vec<Chan>)> {
    let glb = frap_gltf::read(bytes).expect("readable glTF");
    let doc = glb.document;
    let bufs: Vec<Vec<u8>> = glb.bin.into_iter().collect();
    doc.animations
        .iter()
        .map(|a| {
            let chans = a
                .channels
                .iter()
                .map(|c| {
                    let s = &a.samplers[c.sampler as usize];
                    let node = c
                        .target
                        .node
                        .and_then(|n| doc.nodes.get(n as usize))
                        .and_then(|n| n.name.clone())
                        .unwrap_or_default();
                    Chan {
                        node,
                        path: format!("{:?}", c.target.path),
                        interpolation: s.interpolation_or_default(),
                        times: read_accessor(&doc, &bufs, s.input).unwrap().to_f32(),
                        outputs: doc.accessors[s.output as usize].count,
                    }
                })
                .collect();
            (a.name.clone().unwrap_or_default(), chans)
        })
        .collect()
}

fn compare(what: &str, frap: &[(String, Vec<Chan>)], blender: &[(String, Vec<Chan>)]) {
    let names = |v: &[(String, Vec<Chan>)]| v.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>();
    assert_eq!(names(frap), names(blender), "{what}: animations");
    let unordered: BTreeMap<&str, usize> = UNORDERED_TAIL
        .iter()
        .filter(|(c, o, _)| what == format!("{c} {o}"))
        .flat_map(|(_, _, a)| a.iter().copied())
        .collect();
    for ((name, f), (_, b)) in frap.iter().zip(blender) {
        let labels = |v: &[Chan]| v.iter().map(Chan::label).collect::<Vec<_>>();
        let (mut fl, mut bl) = (labels(f), labels(b));
        if let Some(&n) = unordered.get(name.as_str()) {
            fl[n.min(f.len())..].sort();
            bl[n.min(b.len())..].sort();
        }
        assert_eq!(fl, bl, "{what}: channel order of {name}");
        for bc in b {
            let fc = f
                .iter()
                .find(|c| c.label() == bc.label())
                .unwrap_or_else(|| panic!("{what}: {name} has no {}", bc.label()));
            assert_eq!(
                fc.interpolation,
                bc.interpolation,
                "{what}: {name} {} interpolation",
                bc.label()
            );
            assert_eq!(
                fc.times.len(),
                bc.times.len(),
                "{what}: {name} {} key count",
                bc.label()
            );
            for (x, y) in fc.times.iter().zip(&bc.times) {
                assert!(
                    (x - y).abs() <= 1e-6,
                    "{what}: {name} {} times {:?} != {:?}",
                    bc.label(),
                    fc.times,
                    bc.times
                );
            }
            assert_eq!(
                fc.outputs,
                bc.outputs,
                "{what}: {name} {} output count",
                bc.label()
            );
        }
    }
}

#[test]
fn animation_channels_match_blender_structurally() {
    let repo = Repo::locate();
    let props = Props::load(&repo.props_path()).expect("props.toml");
    let cases_dir = repo.cases_dir();
    let dir = cases_dir.join("gltf/animation");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .expect("animation cases")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "py"))
        .collect();
    files.sort();
    let mut checked = 0;
    for path in files {
        let case = Case::load(&path, &cases_dir).expect("case parses");
        let outs: Vec<String> = case
            .compare
            .iter()
            .filter_map(|c| match c {
                CompareSpec::Glb(f) => Some(f.clone()),
                _ => None,
            })
            .collect();
        if outs.is_empty() {
            continue;
        }
        let work = ScratchDir::new(&case.name).unwrap();
        run_frap(
            &case,
            &props,
            &RunOptions {
                workdir: work.path().to_owned(),
                strict_rna: false,
            },
        )
        .unwrap_or_else(|e| panic!("{}: {e}", case.name));
        for out in outs {
            let fixture = repo
                .fixtures_dir()
                .join(&case.area)
                .join(fixture_file_name(&case, &out));
            let blender =
                std::fs::read(&fixture).unwrap_or_else(|e| panic!("{}: {e}", fixture.display()));
            let frap = std::fs::read(work.path().join(Path::new(&out)))
                .unwrap_or_else(|e| panic!("{} {out}: {e}", case.name));
            compare(
                &format!("{} {out}", case.name),
                &animations(&frap),
                &animations(&blender),
            );
            checked += 1;
        }
    }
    assert!(checked >= 20, "only {checked} outputs checked");
}
