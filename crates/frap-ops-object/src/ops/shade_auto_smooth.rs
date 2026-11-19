// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.shade_auto_smooth(use_auto_smooth=True, angle=0.523599)`, tier T1. Port of
//! `shade_auto_smooth_exec` and `is_smooth_by_angle_modifier` (`editors/object/object_edit.cc`,
//! Blender 5.2.4, GPL-2.0-or-later).
//!
//! Verified on 5.2.4 (`-b --factory-startup`):
//! - the first call links the Essentials "Smooth by Angle" group: two `bpy.data.libraries`
//!   entries with the same file path, `geometry_nodes_essentials.blend` and the archive
//!   `geometry_nodes_essentials.blend.001`, and the group (`library` = the archive,
//!   `is_linked_packed`, read-only); later calls reuse it;
//! - every selected editable mesh loses `sharp_face` (`mesh_smooth_set(…, true, true)`, sharp
//!   edges kept); each selected object without a smooth-by-angle modifier gets a NODES modifier
//!   named `Smooth by Angle` (unique), pinned last, `show_group_selector` and
//!   `show_manage_panel` off, its inputs from the group (`Input_0`, `Input_1`, `Socket_1`);
//!   `Input_1` (Angle) is set to `angle` on new and existing ones;
//! - a selected camera gets the WARNING `Modifiers cannot be added to object 'Camera'`; the
//!   operator still finishes;
//! - `use_auto_smooth=False` removes the first smooth-by-angle modifier of each selected
//!   object; the group stays (0 users).
//!
//! A local group named "Smooth by Angle" is not the Essentials one: it is neither reused nor
//! evaluated (the NODES modifier dispatches on the library and the name).

use frap_modifiers::types::nodes::is_smooth_by_angle_modifier;
use frap_modifiers::{ModifierType, stack::edit};
use frap_nodes::NodeTree;
use frap_nodes::builtin_groups::smooth_by_angle as sba;
use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{IdRef, LibraryId, MeshId, NodeTreeId};
use frap_scene::id::{GnInput, GnInputs, GnValue, Library, ModifierSettings};
use frap_scene::{ChangeLog, Main};
use frap_spec_ops_object::object;
use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::support::shade::{SHADE_POLL, smooth_set};

/// The linked Essentials group, when it was imported already
/// (`asset_local_id_ensure_imported` reuses it).
fn existing_group(main: &Main) -> Option<IdRef<NodeTreeId>> {
    main.node_groups.iter().find_map(|(r, t)| {
        let lib = t
            .header
            .library
            .and_then(|l| main.libraries.get(l))
            .map(|l| l.filepath.as_str());
        let valid = t
            .interface
            .iter()
            .filter(|s| s.in_out == frap_nodes::InOut::Input)
            .count()
            == 3
            && t.interface
                .iter()
                .filter(|s| s.in_out == frap_nodes::InOut::Output)
                .count()
                == 1;
        (sba::is_smooth_by_angle(&t.header.name, lib) && valid).then_some(r)
    })
}

/// Links the group as Blender's asset import does: the library, its packed archive, and the
/// group from the archive.
fn link_group(main: &mut Main, log: &mut ChangeLog) -> IdRef<NodeTreeId> {
    let find_lib =
        |main: &Main, name: &str| -> Option<IdRef<LibraryId>> { main.libraries.find(name) };
    if find_lib(main, sba::LIBRARY_BASENAME).is_none() {
        main.libraries.insert(
            sba::LIBRARY_BASENAME,
            Library::new("", sba::ESSENTIALS_FILEPATH),
            log,
        );
    }
    let archive = match find_lib(main, sba::ARCHIVE_LIBRARY_NAME) {
        Some(l) => l,
        None => {
            let mut lib = Library::new("", sba::ESSENTIALS_FILEPATH);
            lib.is_archive = true;
            main.libraries.insert(sba::ARCHIVE_LIBRARY_NAME, lib, log)
        }
    };
    let tree: NodeTree = sba::build(archive);
    main.node_groups.insert(sba::NAME, tree, log)
}

/// `MOD_nodes_update_interface` for a freshly assigned group: every group input with its
/// default value.
fn interface_inputs(main: &Main, group: IdRef<NodeTreeId>) -> GnInputs {
    let mut inputs = IndexMap::new();
    if let Some(t) = main.node_groups.get(group) {
        for s in t
            .interface
            .iter()
            .filter(|s| s.in_out == frap_nodes::InOut::Input)
        {
            let v = match s.default_value {
                frap_nodes::SocketValue::Float(f) => GnValue::Float(f),
                frap_nodes::SocketValue::Bool(b) => GnValue::Bool(b),
                frap_nodes::SocketValue::Int(i) => GnValue::Int(i),
                _ => GnValue::None,
            };
            inputs.insert(s.identifier.clone(), GnInput::new(v));
        }
    }
    GnInputs(inputs)
}

#[operator(
    poll = SHADE_POLL,
    tier = T1,
    notes = [
        "the Essentials library path is <frap>/datafiles/assets/nodes/geometry_nodes_essentials.blend (Blender: its installation)",
    ]
)]
fn exec(cx: &mut OpCtx, p: object::ShadeAutoSmooth) -> OpResult {
    let objects = cx.ctx().selected_editable_objects();
    if !p.use_auto_smooth.value {
        for ob in objects {
            let i = {
                let main = cx.main();
                main.objects.get(ob).and_then(|o| {
                    o.modifiers
                        .iter()
                        .position(|m| is_smooth_by_angle_modifier(main, m))
                })
            };
            if let Some(i) = i {
                edit::remove(&mut cx.object_mut(ob)?.modifiers, i);
            }
        }
        return Ok(OpStatus::Finished);
    }
    let group = match existing_group(cx.main()) {
        Some(g) => g,
        None => {
            let (main, log) = cx.main_mut();
            link_group(main, log)
        }
    };
    let angle = p.angle.value;
    for ob in objects {
        let Some(o) = cx.main().objects.get(ob) else {
            continue;
        };
        // `mesh_smooth_set(mesh, true, true)`: only `sharp_face` goes.
        if let Some(me) = o.data.and_then(|d| d.typed::<MeshId>())
            && cx
                .main()
                .meshes
                .get(me)
                .is_some_and(|m| m.attributes().get(frap_mesh::SHARP_FACE).is_some())
        {
            smooth_set(cx.mesh_mut(me)?, true, true);
        }
        let mut o = cx
            .main()
            .objects
            .get(ob)
            .cloned()
            .expect("invariant: found");
        let existing = {
            let main = cx.main();
            o.modifiers
                .iter()
                .position(|m| is_smooth_by_angle_modifier(main, m))
        };
        let i = match existing {
            Some(i) => i,
            None => {
                let i = match edit::add(&mut o, None, ModifierType::Nodes) {
                    Ok(i) => i,
                    Err(e) => {
                        cx.report(ReportType::Warning, e.to_string());
                        continue;
                    }
                };
                let inputs = interface_inputs(cx.main(), group);
                let m = &mut o.modifiers[i];
                m.use_pin_to_last = true;
                if let ModifierSettings::Nodes(s) = &mut m.settings {
                    s.node_group = Some(group.any());
                    s.inputs = inputs;
                    s.show_group_selector = false;
                    s.show_manage_panel = false;
                }
                o.modifiers[i].name = SmolStr::default();
                let name = edit::unique_name(&o.modifiers, Some(i), sba::NAME, "NODES");
                o.modifiers[i].name = name;
                i
            }
        };
        if let ModifierSettings::Nodes(s) = &mut o.modifiers[i].settings
            && let Some(input) = s.inputs.0.get_mut(sba::INPUT_ANGLE)
        {
            input.value = GnValue::Float(angle);
        }
        *cx.object_mut(ob)? = o;
    }
    Ok(OpStatus::Finished)
}
