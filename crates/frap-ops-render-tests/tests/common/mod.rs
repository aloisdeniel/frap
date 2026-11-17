// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared helpers of the render tests: sessions in scratch directories and the GPU policy of
//! `.github/workflows/{ci,gpu}.yml` (`FRAP_GPU_OPTIONAL=1`: a missing adapter is a visible skip;
//! otherwise it fails).

#![allow(dead_code)] // each test binary uses a subset

use std::path::{Path, PathBuf};

use frap_golden::blender::ScratchDir;
use frap_session::{Response, Session, SessionOptions};
use serde_json::Value as J;

/// Whether a missing adapter is a skip (`FRAP_GPU_OPTIONAL=1`) rather than a failure.
pub fn gpu_optional() -> bool {
    std::env::var("FRAP_GPU_OPTIONAL").is_ok_and(|v| v == "1")
}

/// A session whose cwd is a fresh scratch directory (removed on drop).
pub struct Env {
    /// The session.
    pub session: Session,
    /// Its cwd.
    pub dir: ScratchDir,
}

impl Env {
    /// Factory startup, cwd = a new scratch directory.
    pub fn new(tag: &str) -> Env {
        let dir = ScratchDir::new(tag).expect("scratch dir");
        let mut opts = SessionOptions::default();
        opts.cwd = dir.path().to_owned();
        let session = Session::new(opts).expect("session");
        Env { session, dir }
    }

    /// The cwd.
    pub fn cwd(&self) -> &Path {
        self.dir.path()
    }

    /// A path in the cwd.
    pub fn path(&self, rel: &str) -> PathBuf {
        self.dir.path().join(rel)
    }

    /// Runs one statement and returns its response as JSON.
    pub fn run(&mut self, src: &str) -> J {
        let r = self.session.exec_source(src);
        assert_eq!(r.len(), 1, "{src}: one statement expected");
        r[0].to_json()
    }

    /// Runs statements that must succeed.
    pub fn ok(&mut self, src: &str) {
        for r in self.session.exec_source(src) {
            assert!(r.ok(), "{}", r.to_json());
        }
    }

    /// Runs a render statement; `None` (after printing a notice) when there is no GPU and
    /// `FRAP_GPU_OPTIONAL=1`, a panic when there is none otherwise.
    pub fn render(&mut self, src: &str) -> Option<J> {
        let j = self.run(src);
        if gpu_unavailable(&j) {
            skip_or_fail(&j);
            return None;
        }
        Some(j)
    }
}

/// Whether a response is `GPU_UNAVAILABLE`.
pub fn gpu_unavailable(j: &J) -> bool {
    j["error"]["code"] == "GPU_UNAVAILABLE"
}

/// Prints the skip notice, or panics when the GPU is required.
pub fn skip_or_fail(j: &J) {
    let msg = j["error"]["message"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    if gpu_optional() {
        eprintln!("SKIP (FRAP_GPU_OPTIONAL=1): {msg}");
    } else {
        panic!("no GPU adapter (set FRAP_GPU_OPTIONAL=1 to skip): {msg}");
    }
}

/// Whether any response of a run is `GPU_UNAVAILABLE` (then skipped or failed).
pub fn any_gpu_unavailable(rs: &[Response]) -> bool {
    match rs.iter().map(Response::to_json).find(gpu_unavailable) {
        Some(j) => {
            skip_or_fail(&j);
            true
        }
        None => false,
    }
}

/// Report messages of a response.
pub fn reports(j: &J) -> Vec<(String, String)> {
    j["reports"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|r| {
            (
                r["type"].as_str().unwrap_or_default().to_owned(),
                r["message"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect()
}

/// Asserts two state dumps are equal, showing the first difference.
pub fn assert_same_state(after: &str, before: &str, what: &str) {
    if after == before {
        return;
    }
    let i = after
        .bytes()
        .zip(before.bytes())
        .position(|(a, b)| a != b)
        .unwrap_or(after.len().min(before.len()));
    let win = |s: &str| {
        let a = s.floor_char_boundary(i.saturating_sub(300));
        let b = s.ceil_char_boundary((i + 200).min(s.len()));
        s[a..b].to_owned()
    };
    panic!(
        "{what}: the state changed\n--- after\n…{}…\n--- before\n…{}…",
        win(after),
        win(before)
    );
}

/// The session state, for [`assert_unchanged`]: a clone of `Main` (sharing its mesh `Arc`s, so
/// lazily filled mesh caches, which are not state, read the same on both sides) and the rest of
/// `Session::state_dump`.
pub fn state(s: &Session) -> (impl std::fmt::Debug + use<>, String) {
    let rest = s
        .state_dump()
        .split_once('\n')
        .map(|(_, r)| r.to_owned())
        .unwrap_or_default();
    (s.main().clone(), rest)
}

/// Asserts the state equals one taken with [`state`].
pub fn assert_unchanged(s: &Session, before: &(impl std::fmt::Debug, String), what: &str) {
    let now = state(s);
    assert_same_state(&format!("{:?}", now.0), &format!("{:?}", before.0), what);
    assert_same_state(&now.1, &before.1, what);
}
