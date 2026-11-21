// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-ops-render` (layer L10): `render.render` with Blender's output path rules and `frap.render_contact_sheet`; reaches the GPU only through `OpCtx::services().render()` (ARCHITECTURE §19.4, §19.6). No primary namespace.
//!
//! One file per operator in `src/ops/` (`ops/<op>.rs` for the primary namespace,
//! `ops/<namespace>__<op>.rs` otherwise, §21.1), included with `automod` and registered in
//! `frap_op::OPERATORS`: adding an operator needs no `mod` edit. Only pure unit tests live
//! here; behaviour tests go to `frap-golden` cases or `crates/frap-ops-<x>-tests` (rule 2.2.11).
#![deny(unsafe_code)]

#[allow(non_snake_case)]
mod ops {
    automod::dir!("src/ops");
}

/// Output path rules (`//`, `#` padding, extensions) and image writing (WP 1.24).
pub mod output;

/// D-52: reports, as WARNINGs of the rendering operator, the modifiers of the rendered mesh
/// objects whose result exceeded `--max-elements` and passed the mesh through. Reads the
/// render-mode results `frap_render_scene::build` just cached (the objects `req` renders).
pub(crate) fn report_limit_warnings(
    cx: &mut frap_op::OpCtx,
    host: &dyn frap_scene::HostRead,
    eval: &mut dyn frap_scene::Evaluator,
    req: &frap_render_scene::RenderRequest,
) {
    let ctx = host.ctx();
    let main = host.main();
    let obs =
        frap_render_scene::build::visibility::render_objects(main, ctx.scene(), ctx.view_layer());
    for r in obs {
        let Some(ob) = main.objects.get(r) else {
            continue;
        };
        if ob.object_type() != "MESH"
            || req
                .objects
                .as_ref()
                .is_some_and(|names| !names.contains(&ob.header.name))
        {
            continue;
        }
        if let Ok(eo) = eval.object(host, r, frap_scene::EvalMode::Render) {
            cx.report_limit_warnings(&eo);
        }
    }
}

/// Referenced by `frap_ops_all::link()` so the linker keeps this crate's `linkme` entries (§10.5).
#[inline(never)]
pub fn link() {}
