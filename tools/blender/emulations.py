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
[MESH App. B.7] in M2). Owned by WP 1.8 in M1; in M2 each WP adds the emulation of the frap
operator or headless-extended operator it implements (`select_elements`: WP 2.2,
`select_by_normal`: WP 2.4, `loopcut`: WP 2.6), appending to `EMULATED`.
"""

import bpy


def _edit_bmesh():
    import bmesh

    ob = bpy.context.edit_object
    if ob is None or ob.type != "MESH":
        raise RuntimeError("Operator bpy.ops.frap.select_elements.poll() failed, context is incorrect")
    return ob, bmesh, bmesh.from_edit_mesh(ob.data)


def select_elements(type="VERT", indices=(), action="SET", object=""):
    """`frap.select_elements`: select mesh elements by index in Edit Mode (bmesh selection).

    Mirrors spec/frap/ops/frap.select_elements.json (M2 W0): `action` SET (exactly these, the
    selection history restarts), ADD, REMOVE; `object` names the edit object the indices refer
    to (default: the active object)."""
    import bmesh

    for name, value, items in (("type", type, ("VERT", "EDGE", "FACE")),
                               ("action", action, ("SET", "ADD", "REMOVE"))):
        if value not in items:
            raise TypeError(
                "Converting py args to operator properties: enum \"%s\" not found in %s"
                % (value, str(items))
            )
    if object:
        ob = bpy.data.objects.get(object)
        if ob is None or ob.mode != "EDIT" or ob.type != "MESH":
            raise RuntimeError("Error: frap.select_elements: object \"%s\" is not a mesh in Edit Mode" % object)
        bm = bmesh.from_edit_mesh(ob.data)
    else:
        ob, bmesh, bm = _edit_bmesh()
    seq = {"VERT": bm.verts, "EDGE": bm.edges, "FACE": bm.faces}[type]
    seq.ensure_lookup_table()
    if action == "SET":
        for elems in (bm.verts, bm.edges, bm.faces):
            for e in elems:
                e.select_set(False)
        bm.select_history.clear()
    for i in indices:
        if i < 0 or i >= len(seq):
            raise IndexError("frap.select_elements: index %d out of range (%d)" % (i, len(seq)))
        e = seq[i]
        e.select_set(action != "REMOVE")
        if action != "REMOVE":
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
