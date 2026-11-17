// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared helpers of the `frap-ops-io` behaviour tests: sessions in scratch directories, a
//! statement runner that fails loudly, and a key-independent dump of the session state.
#![allow(dead_code)] // each test binary uses its own subset

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use frap_rna::{IdAny, IdCode, IdRef, IdType};
use frap_scene::{ChangeLog, Main};
use frap_session::{Response, Session, SessionOptions, Startup};

/// A scratch directory removed on drop.
pub struct Scratch(pub PathBuf);

impl Scratch {
    pub fn new(tag: &str) -> Scratch {
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "frap-ops-io-tests-{tag}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }
    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A session (factory or empty startup) whose cwd is `dir`.
pub fn session_in(dir: &Path, empty: bool) -> Session {
    let mut o = SessionOptions::default();
    o.cwd = dir.to_path_buf();
    if empty {
        o.startup = Startup::Empty;
    }
    Session::new(o).unwrap()
}

/// Runs one statement and returns its response.
pub fn run(s: &mut Session, src: &str) -> Response {
    let mut r = s.exec_source(src);
    assert_eq!(r.len(), 1, "{src}: one statement expected");
    r.remove(0)
}

/// Runs statements that must all succeed.
pub fn ok(s: &mut Session, src: &str) -> Vec<Response> {
    let rs = s.exec_source(src);
    for r in &rs {
        assert!(r.ok(), "{src}\n{}", r.to_json());
    }
    rs
}

/// The value of a read (`get`/`call`/`print`).
pub fn get(s: &mut Session, path: &str) -> serde_json::Value {
    let r = run(s, path);
    assert!(r.ok(), "{path}: {}", r.to_json());
    r.value.unwrap_or(serde_json::Value::Null)
}

/// The error code of a failing statement.
pub fn err_code(s: &mut Session, src: &str) -> (String, String) {
    let r = run(s, src);
    let Some(e) = r.error.clone() else {
        panic!("{src} should fail: {}", r.to_json());
    };
    (e.code.0.to_owned(), e.message)
}

/// `Debug` of `bpy.data` and the window with `IdKey`s renumbered by first appearance and
/// generations zeroed: equal for two states that differ only in keys (a save → open).
pub fn normalized(main: &Main, win: &frap_scene::WindowState) -> String {
    let text = format!("{main:?}\n{win:?}");
    let mut out = String::with_capacity(text.len());
    let mut ids: Vec<String> = Vec::new();
    let mut rest = text.as_str();
    let pats = ["IdKey(", ">(", "session_uid: ", "generation: "];
    while let Some((at, pat)) = pats
        .iter()
        .filter_map(|p| rest.find(p).map(|i| (i, *p)))
        .min_by_key(|(i, _)| *i)
    {
        let end = at + pat.len();
        out.push_str(&rest[..end]);
        rest = &rest[end..];
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 {
            continue;
        }
        let n = &rest[..digits];
        if pat == "generation: " {
            out.push('0');
        } else {
            let i = ids.iter().position(|x| x == n).unwrap_or_else(|| {
                ids.push(n.to_owned());
                ids.len() - 1
            });
            out.push_str(&format!("#{i}"));
        }
        rest = &rest[digits..];
    }
    out.push_str(rest);
    out
}

/// Panics with the first differing region of two normalized dumps.
pub fn assert_same_state(loaded: &str, expected: &str, what: &str) {
    if loaded == expected {
        return;
    }
    let (a, b) = (loaded.replace(", ", ",\n"), expected.replace(", ", ",\n"));
    let (la, lb): (Vec<_>, Vec<_>) = (a.lines().collect(), b.lines().collect());
    let i = la
        .iter()
        .zip(&lb)
        .position(|(x, y)| x != y)
        .unwrap_or(la.len().min(lb.len()));
    let lo = i.saturating_sub(15);
    panic!(
        "{what}: states differ at line {i}:\n--- loaded\n{}\n--- expected\n{}",
        la[lo..(i + 5).min(la.len())].join("\n"),
        lb[lo..(i + 5).min(lb.len())].join("\n")
    );
}

/// `main` without the given IDs and with every reference to them cleared: what a save → open
/// of `main` must give, since a save drops orphans and a pointer to a dropped ID reads `None`
/// after loading (verified on Blender 5.2.4: `Light.parent` set to an unlinked object).
pub fn without(main: &Main, ids: &[IdAny]) -> Main {
    let mut m = main.clone();
    let mut log = ChangeLog::default();
    // `IdType::remap_ref` on every ID (not `Main::remap`, which also resyncs view layers).
    for a in ids {
        macro_rules! clear {
            ($($map:ident),*) => {$({
                let keys: Vec<_> = m.$map.iter().map(|(r, _)| r).collect();
                for r in keys {
                    m.$map.get_mut(r, &mut log).unwrap().remap_ref(*a, None);
                }
            })*};
        }
        clear!(
            objects,
            meshes,
            materials,
            cameras,
            lights,
            worlds,
            images,
            armatures,
            actions,
            scenes,
            collections,
            shape_keys,
            node_groups,
            textures,
            libraries,
            pointclouds
        );
    }
    for a in ids {
        macro_rules! rm {
            ($map:ident) => {{
                m.$map.remove(IdRef::from_any(*a).unwrap(), &mut log);
            }};
        }
        match a.code {
            IdCode::OB => rm!(objects),
            IdCode::ME => rm!(meshes),
            IdCode::MA => rm!(materials),
            IdCode::CA => rm!(cameras),
            IdCode::LA => rm!(lights),
            IdCode::WO => rm!(worlds),
            IdCode::IM => rm!(images),
            IdCode::AR => rm!(armatures),
            IdCode::AC => rm!(actions),
            IdCode::SC => rm!(scenes),
            IdCode::GR => rm!(collections),
            IdCode::KE => rm!(shape_keys),
            IdCode::NT => rm!(node_groups),
            IdCode::TE => rm!(textures),
            IdCode::LI => rm!(libraries),
            IdCode::PT => rm!(pointclouds),
            other => panic!("unexpected ID code {other:?}"),
        }
    }
    m
}

/// Normalized state of a session, orphans excluded (what a save keeps).
pub fn saved_state(s: &Session) -> String {
    let main = s.main();
    let kept = without(main, &main.orphans());
    normalized(&kept, s.host().win())
}

/// Normalized state of a session as is.
pub fn state(s: &Session) -> String {
    normalized(s.main(), s.host().win())
}

/// The repository root (`tests/data`, `tests/golden`).
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf()
}
