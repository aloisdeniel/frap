// SPDX-License-Identifier: GPL-3.0-or-later
//! `cargo xtask codegen [--check]` (ARCHITECTURE §4.2): Blender dumps in `spec/blender-5.2/`
//! → `crates/frap-spec-*/src/generated/**` and every `crates/*/src/generated/**`. Owned by
//! WP 1.1. The WP 0.2 prototype (dumps → `StructDef`/`PropDef`/`OperatorSpec` and typed props
//! structs, with `--check` and a drift test) is archived in
//! `docs/design/rfc/0001-probe/codegen/` as a starting point.

/// Entry point. Until WP 1.1 lands, `--check` passes vacuously (nothing is generated yet, the
/// `src/generated/` directories hold hand-written M0 stand-ins checked by drift tests) and a
/// plain run fails.
pub fn run(args: &[String]) -> Result<(), String> {
    if args.iter().any(|a| a == "--check") {
        crate::ci_warning(
            "codegen --check",
            "codegen is not implemented yet (WP 1.1): nothing is checked, this gate is vacuous",
        );
        return Ok(());
    }
    Err("not implemented yet (WP 1.1)".into())
}
