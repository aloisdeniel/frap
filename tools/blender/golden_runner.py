# SPDX-License-Identifier: GPL-3.0-or-later
"""Blender side of the golden harness (ARCHITECTURE §20.2-§20.3). WP 1.8.

    Blender -b --factory-startup --python tools/blender/golden_runner.py -- \\
        case.py out.json [--props tests/golden/props.toml] [--workdir DIR]

Runs the statements of a golden case (the lines both tools run plus the `#@blender` lines; the
`#@frap` lines are blanked so line numbers stay identical), one source line at a time, each
inside `bpy.context.temp_override(window, area=<VIEW_3D>, region=<WINDOW>)` of the window's
Layout screen, records the outcome of every line (ok, or exception class + message; an
operator returning {'CANCELLED'} is not ok), then writes the `GoldenState` JSON through
`dump_state.py`. Relative output paths of the case resolve against `--workdir` (the current
directory by default).

`NORMALISE` lists, per deliberate deviation of `docs/deviations.md` §1, what the golden suite
does about it. The Rust test `normalisation_list_matches_deviations` keeps both in sync: same
IDs, and an entry is `None` exactly when the deviation's Golden column says "none".
"""

import ast
import json
import os
import re
import sys

sys.dont_write_bytecode = True  # no __pycache__ next to the repository's tools

HERE = os.path.dirname(os.path.abspath(__file__))
if HERE not in sys.path:
    sys.path.insert(0, HERE)

try:
    import bpy
except ImportError:  # imported by the Blender-free unit tests (test_golden_runner.py)
    bpy = None
else:
    import dump_state  # noqa: E402
    import emulations  # noqa: E402

# Deviation id -> (who handles it, what is done) or None when nothing is needed.
#   runner:     this file changes how Blender runs the case;
#   dumper:     dump_state.py / the frap dumper encode or skip something;
#   comparator: a frap-golden comparator ignores or normalises something;
#   cases:      golden cases are written so that the deviation never shows;
#   fixtures:   fixtures are blessed so that the deviation never shows (platform, build).
NORMALISE = {
    "D-01": None,
    "D-02": None,
    "D-03": ("comparator", "reports are not part of GoldenState, so they are never compared"),
    "D-04": (
        "runner",
        "view_layer.update() before a statement that reads a derived value and before the final "
        "dump, unless time is dirty (a pending frame_current change is never applied here)",
    ),
    "D-05": ("runner", "update_from_editmode() on objects in Edit Mode before mesh reads and the dump"),
    "D-06": None,
    "D-07": None,
    "D-08": ("cases", "goldens never use // export paths"),
    "D-09": None,
    "D-10": (
        "runner",
        "every statement runs inside temp_override(window, area=VIEW_3D, region=WINDOW) of the Layout screen",
    ),
    "D-11": ("dumper", "'Dots Stroke' is excluded from material dumps and comparisons"),
    "D-12": ("cases", "render goldens never use CYCLES"),
    "D-13": ("dumper", "print lines are recorded by outcome only"),
    "D-14": ("dumper", "non-finite floats are written as {'$float': 'nan' | 'inf' | '-inf'}"),
    "D-15": ("cases", "cases do not compare reprs of shadowed linked IDs"),
    "D-16": None,
    "D-17": None,
    "D-18": None,
    "D-19": ("cases", "UV-sphere topology, UVs and normals are compared at T2 (mesh_canonical): Blender's own edge/face order changes between runs"),
    "D-20": ("cases", "meshes with 1000+ faces compare edges with mesh_canonical"),
    "D-21": None,
    "D-23": None,
    "D-24": ("cases", "cases do not re-include an excluded collection and then compare selection"),
    "D-25": None,
    "D-26": ("comparator", "reports are not part of GoldenState, so they are never compared"),
    "D-27": ("cases", "cases do not call foreach_get/foreach_set on slices"),
    "D-28": ("cases", "cases do not export objects outside the scene"),
    "D-29": None,
    "D-30": None,
    "D-31": None,
    "D-32": None,
    "D-33": ("cases", "cases do not set import_unused_materials on files without images"),
    "D-34": None,
    "D-35": None,
    "D-36": None,
    "D-37": ("comparator", "reports are not part of GoldenState, so they are never compared"),
    "D-38": ("cases", "cases write valid face offsets only"),
    "D-39": ("cases", "cases write valid indices only"),
    "D-40": ("cases", "cases stay inside the hard range"),
    "D-41": None,
    "D-42": None,
    "D-43": None,
    "D-44": ("cases", "cases read collection_uid from scene.collection.session_uid"),
    "D-45": None,
    "D-46": None,
    "D-47": ("dumper", "modifier persistent_uid is not part of GoldenState"),
    "D-48": None,
    "D-49": None,
    "D-50": None,
    "D-51": ("comparator", "reports are not part of GoldenState, so they are never compared"),
    "D-52": None,
    "D-53": None,
    "D-54": ("cases", "cases do not compare select_history after leaving and re-entering Edit Mode"),
    "D-55": None,
    "D-56": None,
    "D-57": None,
    "D-58": None,
    "D-59": None,
    "D-60": None,
    "D-61": ("cases", "cases do not read bpy.context.screen/area/region"),
    "D-62": ("cases", "cases do not compare the repr of space_data"),
    "D-63": ("cases", "cases do not assign unknown studio_light names"),
    "D-64": ("cases", "cases that call select_linked() without delimit never pass an explicit delimit earlier in the same Blender process (the defaults agree then)"),
    "D-65": ("comparator", "the affected cases (16 under edit/extrude/) compare meshes with order=canonical"),
    "D-66": ("cases", "cases do not connect loose edges that touch the edges of more than one face"),
    "D-67": ("fixtures", "fixtures are blessed with Blender on macOS"),
    "D-68": None,
    "D-69": None,
    "D-70": None,
    "D-71": None,
    "D-72": ("cases", "cases do not read attributes.active* after removing attributes past the active index"),
    "D-73": None,
    "D-74": ("cases", "cases set attributes.active before attribute_convert"),
    "D-75": ("cases", "cases do not read attributes.active in Edit Mode after activating a flag attribute"),
    "D-76": ("cases", "cases read active_index in Object Mode only"),
    "D-77": (
        "comparator",
        "n-gon triangulation cases of regular n-gons compare at T2 (canonical order) and "
        "triangulate them before float-accumulating edits",
    ),
    "D-78": ("cases", "cases do not pass unknown mesh.merge types in vertex select mode"),
    "D-79": None,
    "D-80": ("comparator", "the two-bevel cases compare at T2 (canonical order)"),
    "D-81": None,
    "D-82": None,
    "D-83": ("cases", "cases pass correct_uv explicitly when an earlier slide in the same scene set it"),
    "D-84": ("cases", "cases do not call them in Object Mode"),
    "D-85": None,
    "D-86": ("cases", "cases do not read World.users after world.new"),
    "D-87": ("cases", "cases pass socket_type by keyword"),
    "D-88": ("cases", "cases do not re-assign an unchanged colour space after editing pixels"),
    "D-89": ("cases", "cases use sRGB, Linear Rec.709 or Non-Color images"),
    "D-90": ("cases", "cases do not read has_data"),
    "D-91": ("cases", "cases do not compare COLOR_GRID pixels"),
    "D-92": ("comparator", "pixel comparisons use a tolerance above 2.4e-5"),
    "D-93": ("cases", "cases load EXRs as Non-Color"),
    "D-94": ("fixtures", "fixtures are blessed with Blender on macOS; cases avoid negative-scale camera fits"),
    "D-95": ("cases", "cases do not write base flags of objects whose bases in different scenes disagree"),
    "D-96": ("cases", "cases create probe objects in the window's scene"),
    "D-97": None,
    "D-98": ("cases", "cases set only the property of the deform method"),
    "D-99": ("comparator", "LAPLACIANSMOOTH cases pin tol=1e-4, strong_lambda tol=5e-4"),
    "D-100": ("comparator", "these configurations are checked at T3 (KNOWN_GAPS in frap-modifiers/src/types/boolean/tests.rs), not golden cases"),
    "D-101": ("comparator", "EXACT cases compare at T3 (invariants)"),
    "D-102": None,
    "D-103": None,
    "D-104": None,
    "D-105": None,
    "D-106": None,
    "D-107": None,
    "D-108": None,
    "D-109": None,
    "D-110": ("runner", "the Blender side looks through the camera (#@blender view_perspective = 'CAMERA')"),
    "D-111": None,
    "D-112": None,
    "D-113": ("cases", "cases have one image texture node per material"),
    "D-114": ("comparator", "reports are not compared"),
    "D-115": ("comparator", "checked at T3 (uv/pack/aabb_rotate_any)"),
    "D-116": ("comparator", "checked at T3 (uv/lightmap/pack_in_one)"),
    "D-117": None,
    "D-118": ("cases", "gltf/m3_workflows/export_options.py drops texCoord -1 from Blender's no-UV re-export (#@blender line)"),
    "D-119": ("comparator", "gltf/m3_workflows/modifier_stack_reimport.py compares at tol=2e-4"),
    "D-120": ("comparator", "checked at T3 (modifiers/bevel_decimate/decimate_after_subsurf, dissolve_after_subsurf)"),
    "D-121": None,
    "D-122": ("comparator", "stacks with these modifiers compare normals at tol=1e-4"),
    "D-123": None,
    "D-124": None,
    "D-125": ("cases", "soft-shadow EEVEE cases set use_shadow_jitter (render_eevee/core/shadows_area_soft); shadows_point_soft has a loose tolerance"),
    "D-126": ("cases", "cases do not extrude bones with nothing selected"),
    "D-127": ("cases", "cases stay inside the hard range"),
    "D-128": ("cases", "cases do not animate hide_viewport, update non-window view layers, or rename view layers between frame changes"),
    "D-129": ("cases", "cases do not read layer collections, selection or the context object between an unlink and the link"),
    "D-130": None,
    "D-131": None,
    "D-132": ("comparator", "gltf/draco goldens compare axis-aligned or lossless meshes; the rest within 1e-6 in import/decompress/draco.rs"),
    "D-133": ("cases", "KTX2 cases import files with fallbacks (gltf/textures/ktx2_import); KTX2-only files are frap-only"),
    "D-134": ("cases", "cases do not turn KHR_materials_variants_ui off after using the variant properties"),
    "D-135": ("cases", "cases set a variant's variant_idx before its name"),
    "D-136": ("comparator", "gltf_canonical inlines variant mapping materials and variant names"),
    "D-137": ("cases", "variant reset cases export one scene"),
    "D-138": ("cases", "cases do not instance a mesh with morph targets"),
    "D-139": ("cases", "gltf/instancing/animated keeps the quirk (frap_compat='BLENDER')"),
    "D-140": ("cases", "cases with variants use other modifier types under export_apply"),
    "D-141": ("cases", "cases do not write unknown attributes"),
    "D-142": ("cases", "cases do not use such names, read at most one generic attribute as a UV map, drop iridescence channels next to other pointer channels, or export or import pointer channels Blender fails on; reports are not compared"),
    "D-143": ("cases", "cases export the fcurves path with meshopt only without shape-key animation"),
    "D-144": ("cases", "cases use Draco normal quantization 0 or 2..30 and finite meshes"),
    "D-145": ("cases", "FLOAT goldens compare at T2 and use inputs with one canonical result over several Blender runs"),
    "D-146": ("cases", "gltf/subsurf_split/mirror_stack writes Blender's smart_project UVs literally (foreach_set) instead of calling uv.smart_project"),
    "D-147": ("cases", "cases mirror one mesh per run or meshes with different element counts"),
    "D-148": ("cases", "vertex_group_copy_to_selected only fails where Blender changes nothing"),
    "D-149": ("cases", "undo cases push the steps frap pushes implicitly with #@blender bpy.ops.ed.undo_push(...) lines"),
    "D-150": ("cases", "normals_tools COPY only fails where the state after it is the same in both tools"),
    "D-151": ("cases", "cases do not point normals with nothing selected on a mesh without custom_normal"),
    "D-152": ("cases", "cases do not create __mod_weightednormals_faceweight with another type"),
    "D-153": ("cases", "rip cases run mesh.rip only in frap (#@frap) and rebuild the interactive Blender result with #@blender bmesh lines (split_edges / vert_separate and the recorded selection)"),
    "D-154": ("cases", "Blender runs a #@blender transcription of select_grouped, and collection_objects_select in a #@blender temp_override(collection=...)"),
    "D-155": ("cases", "cases only cancel or fail the object selection operators with nothing selected or with extend=True"),
    "D-156": ("cases", "cases do not read users after duplicates_make_real"),
    "D-157": ("cases", "armature/edit_m8/collections keeps its failing assign_to_collection last"),
    "D-158": ("cases", "cases remove the Hook before modifiers_copy_to_selected"),
    "D-159": ("cases", "Blender runs material.copy / material.paste in a #@blender temp_override(material=bpy.context.object.active_material)"),
    "D-160": ("render goldens", "Blender's reference for render.opengl is render.render with BLENDER_WORKBENCH (#@opengl-blender lines)"),
    "D-161": ("cases", "cases copy before every pose.paste (the copy buffer lives in the frap process)"),
    "D-162": ("cases", "pose selection cases cancel only with extend=True or where the deselect changes nothing"),
    "D-163": ("cases", "Blender runs anim.channels_bake in a #@blender Graph Editor temp_override and update_tag()s the baked actions before evaluating them"),
    "D-164": ("cases", "cases create the NLA tracks before nla.actionclip_add"),
    "D-165": ("cases", "op_data_transfer raises transfer errors only from calls that transfer nothing"),
    "D-166": ("comparator", "glTF goldens compare PNG images by their decoded pixels; cases do not compare packed_file.size"),
    "D-167": ("cases", "gltf/m8/shared_accessors_draco only exports meshes Blender encodes without reading past its buffers"),
    "D-168": ("cases", "select_similar_region cases avoid regions next to n-gons of five or more sides"),
}

# Evaluation boundaries (§7.3): an operator call, view_layer.update(), frame_set(),
# evaluated_depsgraph_get() apply pending time.
_ORDER = re.compile(r"\b(view_layer\.update|frame_set|evaluated_depsgraph_get)\s*\(|\bbpy\.ops\.")
# Writes of the scene time: plain and augmented assignments of frame_current / frame_float /
# frame_subframe, and setattr(…, 'frame_…', …).
_TIME_WRITE = re.compile(
    r"\bframe_(?:current|float|subframe)\s*(?:[-+*/%@&|^]|//|\*\*|<<|>>)?=(?!=)"
    r"|\bsetattr\s*\([^,]*,\s*[rRbBuU]?['\"]frame_(?:current|float|subframe)['\"]"
)
_MESH_READ = re.compile(
    r"\b(vertices|edges|polygons|loops|loop_triangles|corner_normals|vertex_normals|"
    r"polygon_normals|attributes|uv_layers|foreach_get|meshes)\b"
)


def load_props(path):
    import tomllib

    with open(path, "rb") as f:
        return tomllib.load(f)


def parse_case(text):
    """(header dict, blender-side source with line numbers preserved)."""
    header = {"startup": "factory", "compare": []}
    out = []
    for raw in text.splitlines():
        s = raw.lstrip()
        if s.startswith("#@frap"):
            out.append("")
            continue
        if s.startswith("#@blender"):
            body = s[len("#@blender"):]
            out.append(body[1:].lstrip() if body.startswith(" ") else body.lstrip())
            continue
        if s.startswith("# golden:"):
            for tok in s[len("# golden:"):].split():
                k, _, v = tok.partition("=")
                if k == "startup":
                    header["startup"] = v
                elif k == "compare":
                    for item in v.split(","):
                        if item:
                            kind, _, arg = item.partition(":")
                            header["compare"].append((kind, arg))
                else:
                    header[k] = v or True
        out.append(raw)
    return header, "\n".join(out) + "\n"


def statement_groups(source):
    """[(line, source text of every statement starting on that line)], in order.

    The whole file is parsed when it can be; otherwise each line is parsed on its own (with
    its continuation lines when that makes it complete) so one SyntaxError does not hide the
    other lines' outcomes.
    """
    lines = source.split("\n")
    try:
        tree = ast.parse(source)
    except SyntaxError:
        tree = None
    groups = []
    if tree is not None:
        by_line = {}
        for node in tree.body:
            by_line.setdefault(node.lineno, []).append(node)
        for line in sorted(by_line):
            nodes = by_line[line]
            end = max(n.end_lineno for n in nodes)
            groups.append((line, "\n".join(lines[line - 1:end]), None))
        return groups
    i = 0
    while i < len(lines):
        text = lines[i]
        if not text.strip() or text.lstrip().startswith("#"):
            i += 1
            continue
        done = False
        for j in range(i, len(lines)):
            chunk = "\n".join(lines[i:j + 1])
            try:
                ast.parse(chunk)
            except SyntaxError:
                continue
            groups.append((i + 1, chunk, None))
            i = j + 1
            done = True
            break
        if not done:
            try:
                ast.parse(text)
                err = SyntaxError("invalid syntax")
            except SyntaxError as e:
                err = e
            groups.append((i + 1, text, err))
            i += 1
    return groups


def view3d_override():
    wm = bpy.context.window_manager
    if not wm.windows:
        return {}
    win = wm.windows[0]
    screen = win.screen
    for area in screen.areas:
        if area.type == "VIEW_3D":
            for region in area.regions:
                if region.type == "WINDOW":
                    return {"window": win, "area": area, "region": region}
    return {"window": win}


def exception_record(line, e):
    if isinstance(e, SyntaxError):
        msg = e.msg
    elif len(e.args) == 1 and isinstance(e.args[0], str):
        msg = e.args[0]
    else:
        msg = str(e)
    return {"line": line, "ok": False, "exception": type(e).__name__, "message": msg}


def time_dirty_after(dirty, text, frame_changed):
    """Time-dirty flag after running statement `text` (no Blender needed).

    The last time-related token of the statement decides: a write of frame_current /
    frame_float / frame_subframe (plain, augmented or through setattr) makes time dirty; an
    operator call, view_layer.update(), frame_set or evaluated_depsgraph_get() evaluates it.
    `frame_changed` (the scene frame differs before and after the statement) is the backstop for
    writes no pattern sees (a helper function, a computed attribute name): the time is then
    dirty unless the statement also crossed an evaluation boundary. frame_set and the frame
    operators (screen.frame_offset, verified on Blender 5.2.4: the new frame's animation is
    applied before the operator returns) change the frame and evaluate it.
    """
    writes = [(m.start(), True) for m in _TIME_WRITE.finditer(text)]
    events = writes + [(m.start(), False) for m in _ORDER.finditer(text)]
    if events:
        dirty = max(events)[1]
    if frame_changed and not writes:
        dirty = not _ORDER.search(text)
    return dirty


def frame_state():
    """Scene name -> (frame_current, frame_subframe), compared before/after each statement."""
    try:
        return {s.name: (s.frame_current, s.frame_subframe) for s in bpy.data.scenes}
    except Exception:  # noqa: BLE001 - a statement may leave bpy.data unreadable
        return None


def frames_differ(before, after):
    if before is None or after is None:
        return False
    return any(k in after and after[k] != v for k, v in before.items())


class Runner:
    def __init__(self, props):
        self.time_dirty = False
        self.derived = re.compile(r"\b(%s)\b" % "|".join(map(re.escape, props["derived"]["props"])))

    def edit_objects(self):
        return [ob for ob in bpy.data.objects if ob.mode == "EDIT"]

    def normalise_before(self, text):
        # D-04: derived reads see fresh values, unless time is dirty (§7.3).
        if self.derived.search(text) and not self.time_dirty:
            bpy.context.view_layer.update()
        # D-05: mesh reads of objects in Edit Mode see the edit data.
        if _MESH_READ.search(text):
            for ob in self.edit_objects():
                ob.update_from_editmode()

    def track_time(self, text, before=None, after=None):
        self.time_dirty = time_dirty_after(self.time_dirty, text, frames_differ(before, after))

    def run_group(self, ns, line, text, err):
        if err is not None:
            return exception_record(line, err)
        try:
            tree = ast.parse(text)
        except SyntaxError as e:
            return exception_record(line, e)
        self.normalise_before(text)
        before = frame_state()
        try:
            with bpy.context.temp_override(**view3d_override()):
                result = None
                if len(tree.body) == 1 and isinstance(tree.body[0], ast.Expr):
                    code = compile(ast.Expression(tree.body[0].value), "<case>", "eval")
                    result = eval(code, ns)  # noqa: S307 - golden cases are trusted test input
                else:
                    exec(compile(tree, "<case>", "exec"), ns)  # noqa: S102
        except BaseException as e:  # noqa: BLE001 - every outcome is recorded
            if isinstance(e, (KeyboardInterrupt, SystemExit)):
                raise
            self.track_time(text, before, frame_state())
            return exception_record(line, e)
        self.track_time(text, before, frame_state())
        cancelled = isinstance(result, set) and result == {"CANCELLED"}
        return {"line": line, "ok": not cancelled}

    def run(self, header, source):
        if header.get("startup") == "empty":
            bpy.ops.wm.read_homefile(use_empty=True)
        ns = {"bpy": bpy, "__name__": "__golden__", "__frap_emul__": emulations.NAMESPACE}
        lines = []
        for line, text, err in statement_groups(emulations.rewrite(source)):
            lines.append(self.run_group(ns, line, text, err))
        # Final dump: same normalisations as for a derived/mesh read.
        if not self.time_dirty:
            bpy.context.view_layer.update()
        for ob in self.edit_objects():
            ob.update_from_editmode()
        return lines


def main(argv):
    args = argv[argv.index("--") + 1:] if "--" in argv else []
    if len(args) < 2:
        print(__doc__, file=sys.stderr)
        sys.exit(2)
    case_path, out_path = os.path.abspath(args[0]), os.path.abspath(args[1])
    root = os.path.dirname(os.path.dirname(HERE))
    props_path = os.path.join(root, "tests", "golden", "props.toml")
    workdir = os.getcwd()
    rest = args[2:]
    while rest:
        flag = rest.pop(0)
        if flag == "--props":
            props_path = os.path.abspath(rest.pop(0))
        elif flag == "--workdir":
            workdir = rest.pop(0)
        else:
            raise SystemExit("golden_runner: unknown argument %r" % flag)
    workdir = os.path.abspath(workdir)
    os.makedirs(workdir, exist_ok=True)
    os.chdir(workdir)
    props = load_props(props_path)
    with open(case_path, encoding="utf-8") as f:
        header, source = parse_case(f.read())
    runner = Runner(props)
    lines = runner.run(header, source)
    with bpy.context.temp_override(**view3d_override()):
        state = dump_state.dump(props, header["compare"], lines, workdir)
    tmp = out_path + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(state, f, indent=1, sort_keys=True)
        f.write("\n")
    os.replace(tmp, out_path)


if __name__ == "__main__":
    main(sys.argv)
