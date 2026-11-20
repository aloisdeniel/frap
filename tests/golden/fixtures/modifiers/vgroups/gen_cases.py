# SPDX-License-Identifier: GPL-3.0-or-later
# WP 6.10: writes the golden cases of tests/golden/cases/modifiers/vgroups/ (the vertex-group
# options of the M4 modifiers). Every statement of a case is also run here, in Blender, so the
# weight lines can be computed from the real vertex positions of the primitives: each group
# gets one `VertexGroup.add` line per distinct weight (rounded to a multiple of 0.05). Regenerate with
#
#   Blender -b --factory-startup --python tests/golden/fixtures/modifiers/vgroups/gen_cases.py
#
# then bless the fixtures (`cargo xtask golden bless --filter 'modifiers/vgroups/*'`).
import math
import os

import bpy

HERE = os.path.dirname(os.path.abspath(__file__))
CASES = os.path.normpath(os.path.join(HERE, "../../../cases/modifiers/vgroups"))


def weight_fn(kind):
    """Deterministic weight patterns over the vertex position; `None` leaves it unassigned."""
    clamp = lambda x: max(0.0, min(1.0, x))  # noqa: E731
    if kind == "x_ramp":
        return lambda i, co: clamp((co.x + 1.0) * 0.5)
    if kind == "y_ramp":
        return lambda i, co: clamp((co.y + 1.0) * 0.5)
    if kind == "z_ramp":
        return lambda i, co: clamp((co.z + 1.0) * 0.5)
    if kind == "half":
        return lambda i, co: (1.0 if co.x > 1e-4 else None)
    if kind == "half_y":
        return lambda i, co: (1.0 if co.y > 1e-4 else None)
    if kind == "every_third":
        return lambda i, co: (None if i % 3 == 0 else 0.25 + 0.5 * ((i * 7) % 5) / 4.0)
    if kind == "radial":
        return lambda i, co: clamp(1.0 - math.hypot(co.x, co.y) * 0.6)
    if kind == "corner":
        return lambda i, co: (1.0 if co.x > 1e-4 and co.y > 1e-4 else None)
    if kind == "top":
        return lambda i, co: (1.0 if co.z > 1e-4 else None)
    if kind == "none":
        return lambda i, co: None
    raise ValueError(kind)


class Case:
    def __init__(self, name, comment, tier="T1", order=None, tol=None):
        bpy.ops.wm.read_factory_settings(use_empty=False)
        self.name = name
        self.comment = comment
        self.tier = tier
        self.order = order
        self.tol = tol
        self.lines = []
        self.compare = []

    def run(self, line):
        """Runs `line` here and records it."""
        try:
            exec(line, {"bpy": bpy})
        except Exception as e:  # noqa: BLE001 (context-dependent operators: the runner has one)
            print("%s: %s: %s" % (self.name, line, e))
        self.lines.append(line)

    def prim(self, op, name):
        """Adds a primitive (`op` is the operator call) and names its object `name`."""
        self.run("bpy.ops.mesh.%s" % op)
        self.run("bpy.context.object.name = %r" % name)
        if op == SPHERE:
            # Blender's UV-sphere edge and face order changes between its runs (D-19).
            self.order = "canonical"
        self.compare.append(name)
        return name

    def group(self, ob, group, kind):
        """`vertex_groups.new` plus one `add` line per distinct weight of `kind`."""
        o = bpy.data.objects[ob]
        self.run("bpy.data.objects[%r].vertex_groups.new(name=%r)" % (ob, group))
        fn = weight_fn(kind)
        by_w = {}
        for v in o.data.vertices:
            w = fn(v.index, v.co)
            if w is not None:
                by_w.setdefault(round(round(w * 20) / 20, 2), []).append(v.index)
        for w in sorted(by_w):
            self.run("bpy.data.objects[%r].vertex_groups[%r].add(%r, %r, 'REPLACE')"
                     % (ob, group, by_w[w], w))

    def mod(self, ob, mtype, name="M", **props):
        self.run("bpy.data.objects[%r].modifiers.new(name=%r, type=%r)" % (ob, name, mtype))
        for k, v in props.items():
            self.run("bpy.data.objects[%r].modifiers[%r].%s = %r" % (ob, name, k, v))

    def write(self):
        head = "".join("# %s\n" % l for l in self.comment.strip().split("\n"))
        h = "# golden: startup=factory compare=%s tier=%s" % (
            ",".join("evaluated:" + n for n in self.compare), self.tier)
        if self.order:
            h += " order=" + self.order
        if self.tol:
            h += " tol=" + self.tol
        with open(os.path.join(CASES, self.name + ".py"), "w") as f:
            f.write(head + h + "\nimport bpy\n" + "\n".join(self.lines) + "\n")


GRID = "primitive_grid_add(x_subdivisions=6, y_subdivisions=5, size=2)"
SPHERE = "primitive_uv_sphere_add(segments=12, ring_count=6, radius=1)"
CUBE = "primitive_cube_add(size=2)"
MONKEY = "primitive_monkey_add(size=2)"
CYLINDER = "primitive_cylinder_add(vertices=8, radius=1, depth=2)"
ICO = "primitive_ico_sphere_add(subdivisions=2, radius=1)"


def mask_vertex_group():
    c = Case("mask_vertex_group", """
WP 6.10: MASK in VERTEX_GROUP mode with real weights: the threshold, `invert_vertex_group`,
`use_smooth` (boundary faces cut at the threshold, inverted too), a missing group (unchanged), a
group without weights (empty, unchanged when inverted), weights interpolated by SUBSURF before
the mask, and the default threshold 0 with unassigned vertices.""")
    for n, props in [
        ("A", {"threshold": 0.4}),
        ("B", {"threshold": 0.4, "invert_vertex_group": True}),
        ("C", {"threshold": 0.45, "use_smooth": True}),
        ("D", {"threshold": 0.45, "use_smooth": True, "invert_vertex_group": True}),
        ("E", {"threshold": 0.4, "vertex_group": "Nope"}),
    ]:
        c.prim(GRID, n)
        c.group(n, "G", "x_ramp")
        p = {"vertex_group": "G"}
        p.update(props)
        c.mod(n, "MASK", **p)
    c.prim(GRID, "F")
    c.group("F", "Other", "half")
    c.run("bpy.data.objects['F'].vertex_groups.new(name='Empty')")
    c.mod("F", "MASK", vertex_group="Empty")
    c.prim(GRID, "F2")
    c.run("bpy.data.objects['F2'].vertex_groups.new(name='Empty')")
    c.mod("F2", "MASK", vertex_group="Empty", invert_vertex_group=True)
    c.prim(ICO, "S")
    c.group("S", "G", "z_ramp")
    c.mod("S", "SUBSURF", name="Sub", levels=1)
    c.mod("S", "MASK", vertex_group="G", threshold=0.5, use_smooth=True)
    c.prim(MONKEY, "Mo")
    c.group("Mo", "A", "half")
    c.group("Mo", "B", "every_third")
    c.mod("Mo", "MASK", vertex_group="B")
    c.write()


def mask_armature():
    c = Case("mask_armature", """
WP 6.10: MASK in ARMATURE mode: a vertex is kept when a selected pose bone of the armature has
a vertex group (named like it) weighing it above the threshold; inverted; the selection read when
the mesh is evaluated (bones selected after the modifier was set); `pose.select_all`; an
armature without a pose built (`objects.new`, nothing selected: empty); no armature
(unchanged).""")
    for n in ("A", "B", "C", "D", "E"):
        c.prim(GRID, n)
        c.group(n, "Bone", "x_ramp")
        c.group(n, "B2", "half_y")
        c.group(n, "Other", "every_third")
        c.mod(n, "MASK", mode="ARMATURE", threshold=0.3)
    c.run("bpy.data.objects['B'].modifiers['M'].invert_vertex_group = True")
    c.run("bpy.ops.object.armature_add()")
    c.run("bpy.ops.object.mode_set(mode='EDIT')")
    c.run("bpy.data.armatures['Armature'].edit_bones.new('B2')")
    c.run("bpy.data.armatures['Armature'].edit_bones['B2'].tail = (0, 1, 1)")
    c.run("bpy.data.armatures['Armature'].edit_bones.new('Other')")
    c.run("bpy.data.armatures['Armature'].edit_bones['Other'].tail = (1, 0, 1)")
    c.run("bpy.ops.object.mode_set(mode='POSE')")
    c.run("bpy.ops.pose.select_all(action='DESELECT')")
    c.run("bpy.data.objects['Armature'].pose.bones['B2'].select = True")
    for n in ("A", "B"):
        c.run("bpy.data.objects[%r].modifiers['M'].armature = bpy.data.objects['Armature']" % n)
    c.run("bpy.data.objects['Armature'].pose.bones['Bone'].select = True")
    c.run("bpy.ops.object.mode_set(mode='OBJECT')")
    c.run("bpy.ops.object.armature_add(location=(3, 0, 0))")
    c.run("bpy.ops.object.mode_set(mode='POSE')")
    c.run("bpy.ops.pose.select_all(action='SELECT')")
    c.run("bpy.ops.object.mode_set(mode='OBJECT')")
    c.run("bpy.data.objects['C'].modifiers['M'].armature = bpy.data.objects['Armature.001']")
    c.run("bpy.data.objects.new('Bare', bpy.data.armatures.new('Bare'))")
    c.run("bpy.data.objects['D'].modifiers['M'].armature = bpy.data.objects['Bare']")
    c.write()


def deform_case(name, comment, mtype, specs, base, smooth=False):
    """`specs`: [(object name, primitive, [(group, kind)], props)]; `smooth` shades every
    object smooth first (custom normals of flat faces are not used)."""
    c = Case(name, comment)
    for n, prim, groups, props in specs:
        c.prim(prim, n)
        if smooth:
            c.run("bpy.ops.object.shade_smooth()")
        for g, kind in groups:
            c.group(n, g, kind)
        p = dict(base)
        p.update(props)
        c.mod(n, mtype, **p)
    c.write()


def displace():
    deform_case("displace", """
WP 6.10: DISPLACE scaled by `vertex_group` (no texture: white), inverted, along X, in GLOBAL
space, with a missing group (unscaled) and with unassigned vertices (weight 0).""",
                "DISPLACE", [
                    ("A", SPHERE, [("G", "z_ramp")], {}),
                    ("B", SPHERE, [("G", "z_ramp")], {"invert_vertex_group": True}),
                    ("C", MONKEY, [("G", "x_ramp")], {"direction": "X", "strength": -0.7}),
                    ("D", SPHERE, [("G", "half")], {"space": "GLOBAL", "direction": "RGB_TO_XYZ"}),
                    ("E", SPHERE, [("H", "half")], {}),
                    ("F", GRID, [("G", "every_third")], {"direction": "Z", "mid_level": 0.2}),
                ], {"vertex_group": "G", "strength": 0.5})


def smooth():
    deform_case("smooth", """
WP 6.10: SMOOTH weighted by `vertex_group`, inverted, with unassigned vertices (not moved), a
locked axis and a missing group.""", "SMOOTH", [
        ("A", SPHERE, [("G", "z_ramp")], {}),
        ("B", SPHERE, [("G", "z_ramp")], {"invert_vertex_group": True}),
        ("C", MONKEY, [("G", "every_third")], {"factor": 1.5, "use_y": False}),
        ("D", MONKEY, [("H", "half")], {}),
    ], {"vertex_group": "G", "factor": 0.8, "iterations": 3})


def laplaciansmooth():
    deform_case("laplaciansmooth", """
WP 6.10: LAPLACIANSMOOTH weighted by `vertex_group`, inverted, with unassigned vertices,
without volume preservation and with a missing group.""", "LAPLACIANSMOOTH", [
        ("A", MONKEY, [("G", "x_ramp")], {}),
        ("B", MONKEY, [("G", "x_ramp")], {"invert_vertex_group": True}),
        ("C", SPHERE, [("G", "every_third")], {"use_volume_preserve": False,
                                                "lambda_factor": 0.8}),
        ("D", GRID, [("G", "radial")], {"lambda_border": 0.5}),
        ("E", SPHERE, [("H", "half")], {}),
    ], {"vertex_group": "G", "lambda_factor": 2.0, "iterations": 3})


def cast():
    deform_case("cast", """
WP 6.10: CAST (SPHERE, CYLINDER, CUBOID) scaled by `vertex_group`, inverted, with unassigned
vertices, with a radius and a size, and with a missing group.""", "CAST", [
        ("A", MONKEY, [("G", "half")], {}),
        ("B", MONKEY, [("G", "x_ramp")], {"cast_type": "CYLINDER", "radius": 0.9}),
        ("C", SPHERE, [("G", "z_ramp")], {"cast_type": "CUBOID", "invert_vertex_group": True,
                                          "factor": 1.2}),
        ("D", MONKEY, [("G", "every_third")], {"cast_type": "CUBOID", "size": 0.5,
                                               "use_y": False}),
        ("E", GRID, [("G", "z_ramp")], {"invert_vertex_group": True, "radius": 0.8}),
        ("F", GRID, [("H", "z_ramp")], {}),
    ], {"vertex_group": "G", "factor": 0.7})


def simple_deform():
    deform_case("simple_deform", """
WP 6.10: SIMPLE_DEFORM (TWIST, BEND, TAPER, STRETCH) scaled by `vertex_group`, inverted, with
unassigned vertices, locks and limits, a missing group, and a group without weights (nothing
moves; everything when inverted).""", "SIMPLE_DEFORM", [
        ("A", CYLINDER, [("G", "z_ramp")], {"angle": 1.3}),
        ("B", MONKEY, [("G", "x_ramp")], {"deform_method": "BEND", "deform_axis": "Z",
                                        "angle": 2.0, "limits": (0.2, 0.8)}),
        ("C", MONKEY, [("G", "every_third")], {"deform_method": "TAPER", "deform_axis": "X",
                                               "factor": 0.6, "invert_vertex_group": True,
                                               "lock_y": True}),
        ("D", SPHERE, [("G", "half")], {"deform_method": "STRETCH", "deform_axis": "Y",
                                        "factor": -0.4, "lock_x": True, "lock_z": True,
                                        "limits": (0.1, 0.7)}),
        ("E", SPHERE, [("H", "half")], {"angle": 0.9}),
        ("F", SPHERE, [("G", "none"), ("H", "half")], {"angle": 0.9}),
        ("F2", SPHERE, [("G", "none"), ("H", "half")], {"angle": 0.9,
                                                        "invert_vertex_group": True}),
    ], {"vertex_group": "G", "deform_axis": "Z"})


def solidify(mode):
    tag = "simple" if mode == "EXTRUDE" else "complex"
    c = Case("solidify_" + tag, """
WP 6.10: SOLIDIFY %s (%s) with `vertex_group` (the thickness scaled by
`thickness_vertex_group + weight * (1 - thickness_vertex_group)`), inverted, with unassigned
vertices, `use_flat_faces`, and `shell_vertex_group` / `rim_vertex_group`, whose weights are
checked through a MASK on each group after the solidify.""" % (
        "Simple" if mode == "EXTRUDE" else "Complex", mode), order="canonical")
    base = {"solidify_mode": mode, "thickness": 0.3, "vertex_group": "G"}
    for n, prim, kind, props in [
        ("A", GRID, "x_ramp", {}),
        ("B", GRID, "x_ramp", {"invert_vertex_group": True, "thickness_vertex_group": 0.25}),
        ("C", MONKEY, "every_third", {"offset": 0.0, "thickness_vertex_group": 0.5}),
        ("D", SPHERE, "z_ramp", {"use_flat_faces": True}),
        ("E", GRID, "radial", {"use_flat_faces": True, "offset": 0.5,
                               "use_even_offset": True}),
    ]:
        c.prim(prim, n)
        c.group(n, "G", kind)
        p = dict(base)
        p.update(props)
        c.mod(n, "SOLIDIFY", **p)
    for n, g in (("Sh", "shell_vertex_group"), ("Ri", "rim_vertex_group")):
        c.prim(GRID, n)
        c.group(n, "G", "x_ramp")
        c.run("bpy.data.objects[%r].vertex_groups.new(name='Out')" % n)
        p = dict(base)
        p[g] = "Out"
        c.mod(n, "SOLIDIFY", **p)
        c.mod(n, "MASK", name="K", vertex_group="Out", threshold=0.5)
    # Both output groups on a closed and an open mesh, the shell inverted by a second mask.
    c.prim(CUBE, "Bo")
    c.run("bpy.data.objects['Bo'].vertex_groups.new(name='S')")
    c.run("bpy.data.objects['Bo'].vertex_groups.new(name='R')")
    c.mod("Bo", "SOLIDIFY", solidify_mode=mode, thickness=0.2, shell_vertex_group="S",
          rim_vertex_group="R")
    c.mod("Bo", "MASK", name="K", vertex_group="S", invert_vertex_group=True)
    c.prim(CYLINDER, "Cy")
    c.run("bpy.data.objects['Cy'].vertex_groups.new(name='S')")
    c.run("bpy.data.objects['Cy'].vertex_groups.new(name='R')")
    c.group("Cy", "G", "top")
    c.mod("Cy", "SOLIDIFY", solidify_mode=mode, thickness=0.2, shell_vertex_group="S",
          rim_vertex_group="R", vertex_group="G")
    c.mod("Cy", "MASK", name="K", vertex_group="S")
    c.write()


def wireframe():
    deform_case("wireframe", """
WP 6.10: WIREFRAME with `vertex_group` (the thickness scaled by
`thickness_vertex_group + weight * (1 - thickness_vertex_group)`), inverted, with unassigned
vertices, even thickness and the original kept, and a missing group.""", "WIREFRAME", [
        ("A", GRID, [("G", "x_ramp")], {}),
        ("B", GRID, [("G", "x_ramp")], {"invert_vertex_group": True,
                                        "thickness_vertex_group": 0.3}),
        ("C", MONKEY, [("G", "every_third")], {"use_even_offset": False,
                                               "use_replace": False}),
        ("D", CUBE, [("G", "half")], {"thickness_vertex_group": 0.5, "offset": 1.0}),
        ("E", CUBE, [("H", "half")], {}),
    ], {"vertex_group": "G", "thickness": 0.1})


def weld():
    deform_case("weld", """
WP 6.10: WELD restricted to `vertex_group` (only vertices with a weight above 0 merge),
inverted, in ALL and CONNECTED modes, and with a missing group (the whole mesh merges,
as without a group).""", "WELD", [
        ("A", GRID, [("G", "x_ramp")], {}),
        ("B", GRID, [("G", "x_ramp")], {"invert_vertex_group": True}),
        ("C", GRID, [("G", "every_third")], {"mode": "CONNECTED"}),
        ("D", ICO, [("G", "half")], {"merge_threshold": 0.6}),
        ("E", ICO, [("G", "half")], {"merge_threshold": 0.6, "mode": "CONNECTED",
                                        "invert_vertex_group": True}),
        ("F", GRID, [("H", "half")], {}),
    ], {"vertex_group": "G", "merge_threshold": 0.45})


def decimate():
    deform_case("decimate", """
WP 6.10: DECIMATE COLLAPSE weighted by `vertex_group` (`vertex_group_factor`), inverted, with
symmetry, and with a missing group.""", "DECIMATE", [
        ("A", MONKEY, [("G", "x_ramp")], {}),
        ("B", MONKEY, [("G", "z_ramp")], {"invert_vertex_group": True,
                                          "vertex_group_factor": 2.0}),
        ("C", MONKEY, [("G", "every_third")], {"use_symmetry": True,
                                               "vertex_group_factor": 0.5}),
        ("D", ICO, [("G", "half")], {"use_collapse_triangulate": True}),
        ("E", ICO, [("H", "half")], {}),
    ], {"vertex_group": "G", "ratio": 0.4})


def bevel():
    deform_case("bevel", """
WP 6.10: BEVEL `limit_method='VGROUP'`: edges whose two vertices weigh at least 0.5 (EDGES) or
those vertices (VERTICES), inverted, with partial weights, and with a missing group (every edge is
bevelled) and a group without weights (none is).""", "BEVEL", [
        ("A", CUBE, [("G", "top")], {}),
        ("B", CUBE, [("G", "top")], {"invert_vertex_group": True}),
        ("C", CUBE, [("G", "corner")], {"affect": "VERTICES"}),
        ("D", CUBE, [("G", "x_ramp")], {"segments": 2}),
        ("E", CUBE, [("H", "top")], {}),
        ("F", CUBE, [("G", "none"), ("H", "top")], {}),
        ("G", MONKEY, [("G", "half")], {"width": 0.05}),
    ], {"limit_method": "VGROUP", "vertex_group": "G", "width": 0.2})


def normal_edit():
    deform_case("normal_edit", """
WP 6.10: NORMAL_EDIT mixing the new normals by `vertex_group` (RADIAL and DIRECTIONAL),
inverted, with unassigned vertices, mix modes and factors, and a missing group.""",
                "NORMAL_EDIT", [
                    ("A", ICO, [("G", "z_ramp")], {}),
                    ("B", ICO, [("G", "z_ramp")], {"invert_vertex_group": True}),
                    ("C", MONKEY, [("G", "every_third")], {"mode": "DIRECTIONAL",
                                                           "mix_factor": 0.7}),
                    ("D", CUBE, [("G", "half")], {"mix_mode": "ADD"}),
                    ("E", GRID, [("G", "x_ramp")], {"mix_mode": "MUL", "offset": (0.3, 0.2, 1.0)}),
                    ("F", ICO, [("H", "half")], {}),
                ], {"vertex_group": "G"}, smooth=True)


def weighted_normal():
    deform_case("weighted_normal", """
WP 6.10: WEIGHTED_NORMAL limited to `vertex_group` (vertices outside keep their normals),
inverted, in FACE_AREA, CORNER_ANGLE and FACE_AREA_WITH_ANGLE modes, and with a missing group.""",
                "WEIGHTED_NORMAL", [
                    ("A", MONKEY, [("G", "half")], {}),
                    ("B", MONKEY, [("G", "half")], {"invert_vertex_group": True}),
                    ("C", CYLINDER, [("G", "top")], {"mode": "CORNER_ANGLE"}),
                    ("D", ICO, [("G", "corner")], {"mode": "FACE_AREA_WITH_ANGLE",
                                                    "keep_sharp": True}),
                    ("E", MONKEY, [("G", "every_third")], {"weight": 80}),
                    ("F", MONKEY, [("H", "half")], {}),
                ], {"vertex_group": "G"}, smooth=True)


def editmode():
    c = Case("editmode", """
WP 6.10: the weights of a mesh in Edit Mode (the BMesh deform layer) reach the modifiers:
weights assigned in Edit Mode (`vertex_group_assign`) and others from Object Mode drive MASK
(`use_smooth`), DISPLACE and SOLIDIFY's shell group (through a MASK) of objects evaluated while
their mesh is in Edit Mode (both objects, multi-object Edit Mode).""")
    c.prim(GRID, "B")
    c.run("bpy.data.objects['B'].vertex_groups.new(name='Out')")
    c.mod("B", "SOLIDIFY", thickness=0.2, shell_vertex_group="Out")
    c.mod("B", "MASK", name="K", vertex_group="Out", invert_vertex_group=True,
          show_in_editmode=True)
    c.prim(GRID, "A")
    c.group("A", "G", "x_ramp")
    c.mod("A", "MASK", vertex_group="G", threshold=0.45, use_smooth=True,
          show_in_editmode=True)
    c.mod("A", "DISPLACE", name="D", vertex_group="G", strength=0.5, show_in_editmode=True)
    c.run("bpy.data.objects['B'].select_set(True)")
    c.run("bpy.ops.object.mode_set(mode='EDIT')")
    c.run("bpy.ops.mesh.select_all(action='DESELECT')")
    c.lines.append("#@frap bpy.ops.frap.select_elements(type='VERT', indices=[0, 1, 2, 8, 9])")
    c.lines.append("#@blender import bmesh; bm = bmesh.from_edit_mesh(bpy.context.object.data); "
                   "bm.verts.ensure_lookup_table(); [setattr(bm.verts[i], 'select', True) "
                   "for i in (0, 1, 2, 8, 9)]; bm.select_flush_mode(); "
                   "bmesh.update_edit_mesh(bpy.context.object.data)")
    c.run("bpy.context.scene.tool_settings.vertex_group_weight = 0.9")
    c.run("bpy.ops.object.vertex_group_assign()")
    c.write()


os.makedirs(CASES, exist_ok=True)
for fn in (mask_vertex_group, mask_armature, displace, smooth, laplaciansmooth, cast,
           simple_deform, lambda: solidify("EXTRUDE"), lambda: solidify("NON_MANIFOLD"),
           wireframe, weld, decimate, bevel, normal_edit, weighted_normal, editmode):
    fn()
