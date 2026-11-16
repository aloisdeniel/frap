// SPDX-License-Identifier: GPL-3.0-or-later
//! Stand-ins for `frap-ops-*` files: operators registered through `linkme` into `OPERATORS`,
//! plus `Session::invoke` (§10.2 steps 1, 3, 5–8, reduced).

use std::sync::Arc;

use frap_probe_op::*;
use frap_probe_render_scene::{RenderRequest, build};
use frap_probe_rna::*;
use frap_probe_spec as spec;
use frap_probe_spec::ops::mesh::PrimitiveCubeAdd;

use crate::Session;

// ---------------------------------------------------------------- mesh.primitive_cube_add

/// Generated spec + generated typed props struct (§10.4), hand-written exec.
fn primitive_cube_add(cx: &mut OpCtx, a: &OpArgs) -> OpResult {
    let p = PrimitiveCubeAdd::from_args(a);
    // §10.3: unset location -> 3D cursor (is-set semantics through Prop<T>).
    let location = if p.location.is_set { p.location.value } else { cx.ctx().cursor() };
    let mut me = frap_probe_mesh::test_cube("Cube");
    let h = p.size.value / 2.0;
    for v in me.positions_mut() {
        for c in v.iter_mut() {
            *c *= h;
        }
    }
    let (main, log) = cx.main_mut();
    let me = main.meshes.insert("Cube", me, log);
    let ob = cx.add_object(
        "Cube",
        Some(me.any()),
        Placement {
            location,
            rotation: [0.0; 3],
        },
    );
    // §10.4 writes `cx.result("object", ob)`: impossible, `Value` has no ID variant and frap-rna
    // cannot name `OBJECT` (generated in L2) -> F-VAL-4. Probe: a Ptr with the srna spelled out.
    cx.result("object", Value::Ptr(RnaPtr::new(Root::Id(ob.any()), &spec::OBJECT)));
    if p.align.value != spec::ops::mesh::primitive_cube_add::Align::World {
        cx.report(ReportType::Info, "probe: align ignored");
    }
    Ok(OpStatus::Finished)
}

#[allow(unsafe_code)] // RFC 0001 F-LINK-1
#[linkme::distributed_slice(OPERATORS)]
static MESH_OT_PRIMITIVE_CUBE_ADD: OperatorDef = OperatorDef {
    spec: &spec::ops::mesh::OPSPEC_MESH_OT_PRIMITIVE_CUBE_ADD,
    poll: poll::ALWAYS,
    exec: primitive_cube_add,
    kind: OpKind::Normal,
    dyn_enums: &[],
    tier: Tier::T1,
    support: Support::Full,
    notes: &[],
};

// ------------------------------------------------------------------ frap.probe_render

/// frap-only operators have hand-written specs (§2.1.3). `poll_matrix` is meaningless for them
/// (no Blender to dump): F-OP-3 proposes `Option<PollMatrix>`.
pub static FRAP_OT_PROBE_RENDER: OperatorSpec = OperatorSpec {
    idname: "frap.probe_render",
    bl_idname: "FRAP_OT_probe_render",
    label: "Probe Render",
    description: "Render the world colour through Services::render() (WP 0.2)",
    options: OpOptions::REGISTER,
    props: &[],
    macro_steps: &[],
    poll_matrix: PollMatrix {
        plain: [PollCell::Y; 5],
        view3d: [PollCell::Y; 5],
    },
};

fn probe_render(cx: &mut OpCtx, _a: &OpArgs) -> OpResult {
    // §19.4: build from a COW clone (F-OP-7), render through the service (§3.2 GPU path).
    let mut snap = cx.snapshot_at(None)?;
    let scene = build(&*snap.host, &mut *snap.eval, &RenderRequest { resolution: Some([16, 8]) });
    let film = cx.services().render()?.render(&scene)?;
    cx.result("width", Value::Int(film.width as i64));
    cx.result(
        "pixel0",
        Value::FloatArray {
            data: small_f32(&film.rgba[0]),
            shape: ArrayShape::Fixed(4),
            tag: MathTag::None,
        },
    );
    cx.result("objects", Value::Int(scene.objects.len() as i64));
    Ok(OpStatus::Finished)
}

#[allow(unsafe_code)] // RFC 0001 F-LINK-1
#[linkme::distributed_slice(OPERATORS)]
static FRAP_OT_PROBE_RENDER_DEF: OperatorDef = OperatorDef {
    spec: &FRAP_OT_PROBE_RENDER,
    poll: poll::OBJECTMODE,
    exec: probe_render,
    kind: OpKind::Normal,
    dyn_enums: &[],
    tier: Tier::T4,
    support: Support::Full,
    notes: &["WP 0.2 probe"],
};

// -------------------------------------------------------------------- frap.probe_edit

pub static FRAP_OT_PROBE_EDIT: OperatorSpec = OperatorSpec {
    idname: "frap.probe_edit",
    bl_idname: "FRAP_OT_probe_edit",
    label: "Probe Edit",
    description: "Hold a Context and &mut EditState at once (split_edit)",
    options: OpOptions::REGISTER.union(OpOptions::UNDO),
    props: &[],
    macro_steps: &[],
    poll_matrix: PollMatrix {
        plain: [PollCell::Y; 5],
        view3d: [PollCell::Y; 5],
    },
};

fn probe_edit(cx: &mut OpCtx, _a: &OpArgs) -> OpResult {
    let (ctx, edit) = cx.split_edit();
    let mode = ctx.tool_settings().mesh_select_mode;
    let cursor = ctx.cursor();
    let selected = ctx.selected_objects().len();
    for em in edit.meshes.values_mut() {
        let bm = Arc::make_mut(&mut em.bm);
        bm.select_mode = mode;
        bm.verts.push(cursor);
        em.bm_gen = next_gen();
    }
    cx.result("selected", Value::Int(selected as i64));
    Ok(OpStatus::Finished)
}

#[allow(unsafe_code)] // RFC 0001 F-LINK-1
#[linkme::distributed_slice(OPERATORS)]
static FRAP_OT_PROBE_EDIT_DEF: OperatorDef = OperatorDef {
    spec: &FRAP_OT_PROBE_EDIT,
    poll: poll::ALWAYS,
    exec: probe_edit,
    kind: OpKind::Normal,
    dyn_enums: &[],
    tier: Tier::T0,
    support: Support::Full,
    notes: &[],
};

// ------------------------------------------------------------------------------ invoke

impl Session {
    /// §10.2 reduced: lookup, keyword conversion (before poll), poll, transaction, exec.
    pub fn invoke(&mut self, idname: &str, kwargs: Vec<(&str, Input)>) -> Result<(OpStatus, OpOutput), FrapError> {
        let def = lookup(idname).ok_or_else(|| {
            FrapError::from(RnaError::new(
                PyExc::AttributeError,
                ErrorCode("OP_UNKNOWN"),
                format!("Calling operator \"bpy.ops.{idname}\" error, could not be found"),
            ))
        })?;
        let spec = def.spec;
        let mut args = OpArgs::default();
        for p in spec.props {
            args.values.insert(p.identifier, default_value(p));
        }
        for (k, input) in kwargs {
            let i = spec.prop_index(k).ok_or_else(|| {
                FrapError::from(RnaError::new(
                    PyExc::TypeError,
                    ErrorCode("OP_ARG_UNKNOWN"),
                    format!("Converting py args to operator properties:: keyword \"{k}\" unrecognized"),
                ))
            })?;
            let p = &spec.props[i];
            let c = coerce(p, input, &CoerceCx::OpArgs { owner: spec.bl_idname })?;
            args.values.insert(p.identifier, c.value);
            args.is_set.insert(i);
        }
        if let Err(f) = (def.poll.check)(&frap_probe_scene::HostRead::ctx(&self.host)) {
            let msg = f.message.unwrap_or_else(|| "failed, context is incorrect".into());
            return Err(RnaError::new(
                PyExc::RuntimeError,
                ErrorCode("OP_POLL_FAILED"),
                format!("Operator bpy.ops.{idname}.poll() {msg}"),
            )
            .into());
        }
        self.transaction(|s| {
            let mut cx = OpCtx::new(&mut s.host, &mut s.services, spec);
            let r = (def.exec)(&mut cx, &args);
            let out = cx.finish();
            match r {
                Ok(st) => Ok((st, out)),
                Err(e) => Err(RnaError::new(PyExc::RuntimeError, ErrorCode("OP_EXEC_ERROR"), format!("Error: {e:?}")).into()),
            }
        })
    }
}
