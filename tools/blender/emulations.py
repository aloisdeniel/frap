# SPDX-License-Identifier: GPL-3.0-or-later
"""Blender-side emulations of frap-only operators (ARCHITECTURE §20.2, [FID §14.4]). WP 1.8.

A golden case may call a frap-namespaced operator on a line both tools run, e.g.
`bpy.ops.frap.select_elements(type='FACE', indices=[5])`. Blender has no `bpy.ops.frap`, so
`golden_runner.py` rewrites `bpy.ops.frap.<name>(` into `__frap_emul__.<name>(` before running the
statement, and this module implements `<name>` with plain Blender API calls. An operator that
is not in `EMULATED` is left alone (Blender then raises its usual `could not be found`
AttributeError, and the case author must use a `#@blender` line instead).

Each emulation returns what the operator would (`{'FINISHED'}` / `{'CANCELLED'}`) and raises
the exception class Blender would raise for bad input.

Later milestones add entries here (for example the `mesh.loopcut(edge_index=…)` recipe of
[MESH App. B.7] in M2); this file is owned by WP 1.8.
"""

import bpy


def _edit_bmesh():
    import bmesh

    ob = bpy.context.edit_object
    if ob is None or ob.type != "MESH":
        raise RuntimeError("Operator bpy.ops.frap.select_elements.poll() failed, context is incorrect")
    return ob, bmesh, bmesh.from_edit_mesh(ob.data)


def select_elements(type="VERT", indices=(), extend=False, deselect=False):
    """`frap.select_elements`: select mesh elements by index in Edit Mode (bmesh selection)."""
    if type not in ("VERT", "EDGE", "FACE"):
        raise TypeError(
            "Converting py args to operator properties: enum \"%s\" not found in ('VERT', 'EDGE', 'FACE')"
            % type
        )
    ob, bmesh, bm = _edit_bmesh()
    seq = {"VERT": bm.verts, "EDGE": bm.edges, "FACE": bm.faces}[type]
    seq.ensure_lookup_table()
    if not extend and not deselect:
        for elems in (bm.verts, bm.edges, bm.faces):
            for e in elems:
                e.select_set(False)
        bm.select_history.clear()
    for i in indices:
        if i < 0 or i >= len(seq):
            raise IndexError("frap.select_elements: index %d out of range (%d)" % (i, len(seq)))
        e = seq[i]
        e.select_set(not deselect)
        if not deselect:
            bm.select_history.add(e)
    bm.select_flush_mode()
    bmesh.update_edit_mesh(ob.data)
    return {"FINISHED"}


def render_contact_sheet(**_kwargs):
    """`frap.render_contact_sheet` has no Blender equivalent: a no-op that succeeds."""
    return {"FINISHED"}


# frap operator name (in `bpy.ops.frap.*`) -> emulation.
EMULATED = {
    "select_elements": select_elements,
    "render_contact_sheet": render_contact_sheet,
}


class _Namespace:
    """`__frap_emul__` in the statement namespace."""

    def __getattr__(self, name):
        try:
            return EMULATED[name]
        except KeyError:
            raise AttributeError(
                'Calling operator "bpy.ops.frap.%s" error, could not be found' % name
            ) from None


NAMESPACE = _Namespace()


def rewrite(source):
    """Routes emulated `bpy.ops.frap.<name>(` calls of one statement to this module."""
    import re

    def sub(m):
        name = m.group(1)
        return "__frap_emul__.%s(" % name if name in EMULATED else m.group(0)

    return re.sub(r"\bbpy\.ops\.frap\.([A-Za-z_][A-Za-z0-9_]*)\s*\(", sub, source)
