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

import bpy

sys.dont_write_bytecode = True  # no __pycache__ next to the repository's tools

HERE = os.path.dirname(os.path.abspath(__file__))
if HERE not in sys.path:
    sys.path.insert(0, HERE)

import dump_state  # noqa: E402
import emulations  # noqa: E402

# Deviation id -> (who handles it, what is done) or None when nothing is needed.
#   runner:     this file changes how Blender runs the case;
#   dumper:     dump_state.py / the frap dumper encode or skip something;
#   comparator: a frap-golden comparator ignores or normalises something;
#   cases:      golden cases are written so that the deviation never shows.
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
}

_ORDER = re.compile(r"\b(view_layer\.update|frame_set|evaluated_depsgraph_get)\s*\(|\bbpy\.ops\.")
_TIME_WRITE = re.compile(r"\bframe_current\s*=(?!=)")
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

    def track_time(self, text):
        # The last time-related token of the statement decides: `frame_current = …` makes time
        # dirty; an operator call, view_layer.update(), frame_set() or
        # evaluated_depsgraph_get() evaluates it.
        events = [(m.start(), True) for m in _TIME_WRITE.finditer(text)]
        events += [(m.start(), False) for m in _ORDER.finditer(text)]
        if events:
            self.time_dirty = max(events)[1]

    def run_group(self, ns, line, text, err):
        if err is not None:
            return exception_record(line, err)
        try:
            tree = ast.parse(text)
        except SyntaxError as e:
            return exception_record(line, e)
        self.normalise_before(text)
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
            self.track_time(text)
            return exception_record(line, e)
        self.track_time(text)
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
