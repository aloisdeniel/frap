// SPDX-License-Identifier: GPL-3.0-or-later
//! M5 through the real `frap` binary: an EEVEE render with an enormous
//! `eevee.taa_render_samples` (Blender accepts up to 2^31 − 1) must answer `TIMEOUT` about at
//! `--stmt-timeout`, roll back, and leave the session usable (M5+M6 stabilization round 3:
//! building the TAA sample list was O(n²) and outside any cancellation point, so the session
//! hung for minutes to hours).

use std::io::Write;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value as J;

fn run(args: &[&str], stdin: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_frap"))
        .env_remove("FRAP_LOG")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn responses(o: &Output) -> Vec<J> {
    String::from_utf8(o.stdout.clone())
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str::<J>(l).unwrap_or_else(|e| panic!("{e}: {l}")))
        .filter(|l| l.get("seq").is_some())
        .collect()
}

fn huge_taa_render(samples: u32, size: [u32; 2], dir: &std::path::Path) -> String {
    format!(
        "context.scene.render.engine = 'BLENDER_EEVEE'\n\
         context.scene.render.resolution_x = {}\n\
         context.scene.render.resolution_y = {}\n\
         context.scene.eevee.taa_render_samples = {samples}\n\
         context.scene.render.filepath = '{}'\n\
         bpy.ops.render.render(write_still=True)\n\
         context.scene.eevee.taa_render_samples\n",
        size[0],
        size[1],
        dir.join("out.png").display()
    )
}

#[test]
fn huge_taa_render_samples_time_out_at_the_deadline() {
    let dir = std::env::temp_dir().join(format!("frap-m5-cli-taa-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // 2^31 − 1 at a tiny size (the sample list alone used to take hours) and 100000 at a
    // larger one (per-sample GPU work).
    for (samples, size) in [(i32::MAX as u32, [32, 32]), (100_000, [640, 360])] {
        let t = Instant::now();
        let o = run(
            &["--stmt-timeout", "2"],
            &huge_taa_render(samples, size, &dir),
        );
        let elapsed = t.elapsed();
        // A statement error does not fail the session (`--on-error continue`).
        assert_eq!(
            o.status.code(),
            Some(0),
            "stderr: {}",
            String::from_utf8_lossy(&o.stderr)
        );
        let rs = responses(&o);
        assert_eq!(rs.len(), 7, "{rs:?}");
        let r = &rs[5];
        if r["error"]["code"] == "GPU_UNAVAILABLE"
            && std::env::var_os("FRAP_GPU_OPTIONAL").is_some()
        {
            eprintln!("SKIP (FRAP_GPU_OPTIONAL=1): no GPU adapter");
            return;
        }
        assert_eq!(r["error"]["code"], "TIMEOUT", "{r}");
        assert_eq!(r["error"]["detail"]["stmt_timeout_ms"], 2000, "{r}");
        // The deadline plus process start-up and GPU initialisation; never minutes.
        assert!(
            elapsed < Duration::from_secs(15),
            "{samples} samples took {elapsed:?}"
        );
        // The session survives and the scene is unchanged by the rolled-back render.
        assert_eq!(rs[6]["value"], samples, "{}", rs[6]);
        assert!(!dir.join("out.png").exists());
    }
    std::fs::remove_dir_all(&dir).ok();
}
