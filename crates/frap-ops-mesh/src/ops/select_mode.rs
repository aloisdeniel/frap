// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.select_mode(type=…, action='TOGGLE', use_extend=False, use_expand=False)`,
//! tier T1 (`tests/golden/cases/edit/prereq/select_mode_*.py`). Port of
//! `edbm_select_mode_exec` and `EDBM_selectmode_toggle_multi`
//! (`editors/mesh/editmesh_select.cc`, Blender 5.2.4, GPL-2.0-or-later); the per-mesh switch
//! (`EDBM_selectmode_convert` + `EDBM_selectmode_set`, the conversion table of [MESH §3.3]) is
//! `BMesh::select_mode_convert`.
//!
//! The new mode is computed from the **active** edit mesh's mode:
//! - `DISABLE` clears `type` (nothing to do when it is not set or is the only mode),
//!   `ENABLE` adds it (nothing to do when set), `TOGGLE` flips it (nothing to do when it is the
//!   only mode);
//! - without `use_extend` the result is `type` alone, and `use_expand` first converts the
//!   selection from the highest old mode to `type` (expanding upward: vertices → edges or
//!   faces touching them; contracting downward: elements not fully surrounded by selected ones
//!   are deselected);
//! - every mesh in Edit Mode switches (`EDBM_selectmode_set`, which strips the selection
//!   history of elements the new mode cannot select and re-flushes), and
//!   `tool_settings.mesh_select_mode` follows.
//!
//! "Nothing to do" answers `CANCELLED` (Blender is silent; frap adds a WARNING, D-51), so
//! `select_mode(type='VERT')` in vertex-only mode, whose default action is `TOGGLE`, is
//! cancelled: pass `action='ENABLE'` (or `use_extend=False` with another type) to switch.

use frap_bmesh::SelectMode;
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh::{
    self,
    select_mode::{Action, Type},
};

/// `SCE_SELECT_*` of the `type` enum.
fn bit(t: Type) -> SelectMode {
    match t {
        Type::Vert => SelectMode::VERT,
        Type::Edge => SelectMode::EDGE,
        Type::Face => SelectMode::FACE,
    }
}

/// The mode switch `EDBM_selectmode_toggle_multi` computes: `None` when there is nothing to do
/// (`only_update`), else the new mode and whether to convert the selection first
/// (`use_expand` without `use_extend`).
pub(crate) fn plan(
    old: SelectMode,
    toggle: SelectMode,
    action: Action,
    use_extend: bool,
    use_expand: bool,
) -> Option<(SelectMode, bool)> {
    let mut new = old;
    match action {
        Action::Disable => {
            if !old.intersects(toggle) || old == toggle {
                return None;
            }
            new.remove(toggle);
        }
        Action::Enable => {
            if old.intersects(toggle) {
                return None;
            }
            new.insert(toggle);
        }
        Action::Toggle => {
            if old == toggle {
                return None;
            }
            new.toggle(toggle);
        }
    }
    if !use_extend || new.is_empty() {
        return Some((toggle, use_expand));
    }
    Some((new, false))
}

/// `ts->selectmode` as `tool_settings.mesh_select_mode`.
fn bits(m: SelectMode) -> [bool; 3] {
    [
        m.contains(SelectMode::VERT),
        m.contains(SelectMode::EDGE),
        m.contains(SelectMode::FACE),
    ]
}

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    notes = [
        "the default action TOGGLE cancels when type is already the only mode (Blender is silent; frap adds a WARNING)",
        "use_expand without use_extend converts the selection from the highest old mode (vertex -> edge/face expands, face -> vertex/edge contracts)",
    ]
)]
fn exec(cx: &mut OpCtx, p: mesh::SelectMode) -> OpResult {
    let meshes = helpers::iter_edit_meshes(cx);
    let Some(&(_, active)) = meshes.first() else {
        return Ok(OpStatus::Cancelled);
    };
    let old = {
        let (_, edit) = cx.split_edit();
        match edit.meshes.get(&active.key()) {
            Some(em) => em.bm.select_mode,
            None => return Ok(OpStatus::Cancelled),
        }
    };
    let toggle = bit(p.r#type.value);
    let Some((new, expand)) = plan(
        old,
        toggle,
        p.action.value,
        p.use_extend.value,
        p.use_expand.value,
    ) else {
        cx.report(
            ReportType::Warning,
            format!(
                "frap: nothing to change: select_mode(type='{}', action='{}') leaves the select mode as it is",
                p.r#type.value.identifier(),
                p.action.value.identifier()
            ),
        );
        return Ok(OpStatus::Cancelled);
    };
    for (_, me) in meshes {
        cx.bmesh_mut(me)?.select_mode_convert(new, expand);
    }
    let scene = cx.ctx().win.scene;
    let (main, log) = cx.main_mut();
    if let Some(s) = main.scenes.get_mut(scene, log) {
        s.tool_settings.mesh_select_mode = bits(new);
    }
    log.context_changed = true;
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    const V: SelectMode = SelectMode::VERT;
    const E: SelectMode = SelectMode::EDGE;
    const F: SelectMode = SelectMode::FACE;

    #[test]
    fn toggle_switches_to_the_type_alone() {
        assert_eq!(plan(V, E, Action::Toggle, false, false), Some((E, false)));
        assert_eq!(plan(V, F, Action::Toggle, false, true), Some((F, true)));
        assert_eq!(plan(V, V, Action::Toggle, false, false), None);
        // A mixed mode toggles the type off without use_extend… into the type alone.
        assert_eq!(plan(V | E, E, Action::Toggle, false, false), Some((E, false)));
    }

    #[test]
    fn extend_combines_modes() {
        assert_eq!(plan(V, E, Action::Toggle, true, false), Some((V | E, false)));
        assert_eq!(plan(V | E, E, Action::Toggle, true, false), Some((V, false)));
        // use_expand only applies without use_extend.
        assert_eq!(plan(V, F, Action::Enable, true, true), Some((V | F, false)));
    }

    #[test]
    fn nothing_to_do_is_none() {
        assert_eq!(plan(V, E, Action::Disable, false, false), None);
        assert_eq!(plan(V, V, Action::Disable, false, false), None);
        assert_eq!(plan(V | E, E, Action::Enable, false, false), None);
        assert_eq!(plan(V | E, E, Action::Disable, false, false), Some((E, false)));
        assert_eq!(plan(V | E, E, Action::Disable, true, false), Some((V, false)));
    }
}
