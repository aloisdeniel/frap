// SPDX-License-Identifier: GPL-3.0-or-later
//! `--stmt-timeout` latency of the EXACT and MANIFOLD booleans and of the EXACT knife
//! (WP 8.6): on the M5+M6 stabilization round 7 worst case (two 130k-triangle spheres, EXACT
//! answered about 3 s after the deadline in a debug build) the statement unwinds with
//! `Interrupted` within 50 ms of the deadline wherever the deadline falls: in the imports
//! (`Manifold::from_mesh_gl*`, not cancellable), the kernels, the export or frap's assembly.

use std::time::{Duration, Instant};

use frap_boolean::{BoolMesh, BoolOperation, BoolOptions, SolverKind};

type Sphere = (Vec<[f32; 3]>, Vec<u32>, Vec<u32>);

/// A UV sphere (`segments` x `rings`) of triangles and quads.
fn uv_sphere(segments: u32, rings: u32, r: f32, c: [f32; 3]) -> Sphere {
    let mut pos = vec![[c[0], c[1], c[2] + r]];
    for i in 1..rings {
        let th = std::f32::consts::PI * i as f32 / rings as f32;
        for j in 0..segments {
            let ph = 2.0 * std::f32::consts::PI * j as f32 / segments as f32;
            pos.push([
                c[0] + r * th.sin() * ph.cos(),
                c[1] + r * th.sin() * ph.sin(),
                c[2] + r * th.cos(),
            ]);
        }
    }
    pos.push([c[0], c[1], c[2] - r]);
    let bottom = pos.len() as u32 - 1;
    let ring = |i: u32, j: u32| 1 + i * segments + (j % segments);
    let mut offs = vec![0u32];
    let mut cv = Vec::new();
    for j in 0..segments {
        cv.extend([0, ring(0, j), ring(0, j + 1)]);
        offs.push(cv.len() as u32);
    }
    for i in 0..rings - 2 {
        for j in 0..segments {
            cv.extend([
                ring(i, j),
                ring(i + 1, j),
                ring(i + 1, j + 1),
                ring(i, j + 1),
            ]);
            offs.push(cv.len() as u32);
        }
    }
    for j in 0..segments {
        cv.extend([bottom, ring(rings - 2, j + 1), ring(rings - 2, j)]);
        offs.push(cv.len() as u32);
    }
    (pos, offs, cv)
}

/// Two 256 x 256 spheres (130 560 triangles each), overlapping.
fn spheres() -> (Sphere, Sphere) {
    (
        uv_sphere(256, 256, 1.0, [0.0; 3]),
        uv_sphere(256, 256, 1.0, [0.5, 0.3, 0.2]),
    )
}

/// What runs under the deadline.
#[derive(Copy, Clone, Debug)]
enum Work {
    Boolean(SolverKind),
    Knife,
}

/// Runs `work` with `timeout`: how long after the deadline it unwound with `Interrupted`,
/// or `None` when it finished first.
fn overshoot(s: &(Sphere, Sphere), work: Work, timeout: Duration) -> Option<Duration> {
    let (a, b) = s;
    let ops = [
        BoolMesh::new(&a.0, &a.1, &a.2),
        BoolMesh::new(&b.0, &b.1, &b.2),
    ];
    let _scope = frap_math::budget::scope(frap_math::budget::Budget {
        timeout: Some(timeout),
        max_elements: u64::MAX,
    });
    let start = Instant::now();
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match work {
        Work::Boolean(solver) => {
            let mut opts = BoolOptions::default();
            opts.operation = BoolOperation::Difference;
            opts.solver = solver;
            frap_boolean::default_solver().run(&ops, &opts).map(|_| ())
        }
        Work::Knife => frap_boolean::knife::knife(&ops, false).map(|_| ()),
    }));
    let el = start.elapsed();
    match r {
        Err(p) if frap_math::budget::interrupted(p.as_ref()).is_some() => {
            Some(el.saturating_sub(timeout))
        }
        Err(p) => std::panic::resume_unwind(p),
        Ok(r) => {
            r.unwrap();
            None
        }
    }
}

/// The latency bound of the acceptance (§22.10, WP 8.6).
const BOUND: Duration = Duration::from_millis(50);

const WORK: [Work; 3] = [
    Work::Boolean(SolverKind::Exact),
    Work::Boolean(SolverKind::Manifold),
    Work::Knife,
];

#[test]
fn timeout_answers_within_50_ms_of_the_deadline() {
    let s = spheres();
    for work in WORK {
        // 20 ms: in the imports (about 60 ms per sphere in a release build, 700 ms in a debug
        // one); 400 ms and 1.5 s: in the imports or the kernels, depending on the build.
        for ms in [20u64, 400, 1500] {
            // Best of three: a run descheduled on a loaded CI runner overshoots by the time
            // slice it lost (macOS runner, 74 ms once), not by cancellation latency; a missing
            // check in the kernels overshoots on every attempt (decision log M8CI-1).
            let mut worst = Vec::new();
            for _ in 0..3 {
                match overshoot(&s, work, Duration::from_millis(ms)) {
                    Some(o) if o >= BOUND => worst.push(o),
                    _ => break,
                }
            }
            assert!(
                worst.len() < 3,
                "{work:?} at {ms} ms: {worst:?} after the deadline (3 attempts)"
            );
        }
    }
}

#[test]
#[ignore = "slow sweep (`cargo test -p frap-boolean --test cancel -- --ignored`)"]
fn timeout_sweep() {
    let s = spheres();
    for work in WORK {
        let mut ms = 10u64;
        loop {
            let o = overshoot(&s, work, Duration::from_millis(ms));
            eprintln!("{work:?} {ms} ms: {o:?}");
            match o {
                Some(o) => assert!(o < BOUND, "{work:?} at {ms} ms: {o:?}"),
                None => break,
            }
            ms = ms * 3 / 2;
        }
    }
}
