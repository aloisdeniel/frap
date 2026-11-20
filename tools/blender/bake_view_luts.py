# SPDX-License-Identifier: GPL-3.0-or-later
"""Bake Blender's colour management into frap's OCIO op pipelines, LUT data and a held-out colour
set (ARCHITECTURE §19.5).

Runs INSIDE Blender:

    $BLENDER_BIN -b --factory-startup --python tools/blender/bake_view_luts.py -- \
        --out assets/luts [--heldout 400] [--seed 20261116]

Method:

1. Pipelines. For every SDR display of Blender 5.2.4's `config.ocio` (sRGB, Display P3, Rec.1886,
   Rec.2020), every view of that display that frap's `ViewTransform` names, and every look that
   Blender applies to that view (`colormanage_use_look` + `view_filter_for_look`), this script
   builds the very transform Blender builds for a render/save (`create_ocio_display_transform`,
   `imbuf/opencolorio/intern/libocio/libocio_display_processor.cc`: a `LookTransform` into the
   look's process space, then a `DisplayViewTransform` with looks bypassed; exposure and gamma are
   applied by frap around it), asks Blender's bundled OpenColorIO (PyOpenColorIO 2.5) for the
   processor with `OPTIMIZATION_LOSSLESS` (matrices composed, identities removed, nothing
   approximated), and writes its op list as one `[display|view|look]` section of
   `assets/luts/<family>.ops`. `frap-color` interprets those ops with ports of OCIO's CPU
   renderers (`crates/frap-color/src/display/ocio.rs`).
2. LUT data. The 1D/3D LUTs those ops reference are written once each, under the name of the
   config file they come from (`<name>.cube` with red varying fastest, `<name>.lut1d`), read back
   from Blender's own OCIO processors; AgX's base sRGB LUT keeps the `agx.cube` name and its
   `# frap-*:` header so `frap_color::lut::baked("AgX", "None", "sRGB")` still reads it.
3. Held-out set: random HDR colours (fixed seed, scene-linear Rec.709) are pushed through
   `Image.save_render` (16-bit PNG, dither 0) for every baked combination, a few exposure/gamma
   settings and a few looks Blender ignores, and written with Blender's 16-bit display outputs to
   `assets/luts/heldout.json`. frap's tests gate on CIEDE2000 < 1 against these renders only (the
   pipelines are not fitted to anything, so every colour is held out).
4. Dither: `Image.save_render` does not dither (only render results carry
   `render.dither_intensity`), so a flat Workbench render (world colour only) is written twice
   with `render.render(write_still=True)`: as an 8-bit PNG with `dither_intensity = 1` and as a
   16-bit PNG (undithered display values). This pins Blender's noise pattern and amplitude.

Every number written is read from Blender 5.2.4's processors; no OCIO data is typed by hand. The
script is deterministic: re-running it reproduces every file byte for byte.
"""

import json
import os
import random
import struct
import sys
import tempfile
import zlib

import bpy
import PyOpenColorIO as OCIO

CONFIG_DIR = os.path.join(bpy.utils.resource_path("LOCAL"), "datafiles", "colormanagement")
CONFIG = OCIO.Config.CreateFromFile(os.path.join(CONFIG_DIR, "config.ocio"))

# frap's `ViewTransform` items (the sRGB display's views, Blender's enum order) and the module
# (`crates/frap-color/src/<family>.rs`) whose `.ops` file holds them.
FAMILY = {
    "Standard": "standard",
    "ACES 1.3": "aces",
    "ACES 2.0": "aces",
    "Khronos PBR Neutral": "pbr_neutral",
    "AgX": "agx",
    "Filmic": "filmic",
    "Filmic Log": "filmic",
    "False Color": "false_color",
    "Raw": "standard",
}
# The SDR displays (`DisplayDevice`); the HDR ones (Rec.2100-PQ/HLG) re-encode image files to
# the 203-nit convention (`adjust_for_hdr_image_file`) and are not baked.
DISPLAYS = ["sRGB", "Display P3", "Rec.1886", "Rec.2020"]

# config.ocio, colorspace "Linear Rec.709": XYZ D65 -> Linear Rec.709.
XYZ_TO_REC709 = [
    [3.2409699, -1.5373832, -0.4986108],
    [-0.9692436, 1.8759675, 0.0415551],
    [0.0556301, -0.2039770, 1.0569715],
]
# config.ocio, colorspace "Linear FilmLight E-Gamut": XYZ D65 -> Linear E-Gamut.
XYZ_TO_EGAMUT = [
    [1.5250528, -0.3159135, -0.1226583],
    [-0.5091526, 1.3333274, 0.1382844],
    [0.0957153, 0.0508974, 0.7879558],
]
# config.ocio, view transform "AgX Base Rec.1886": AllocationTransform lg2 vars.
AGX_LG2 = (-12.47393, 12.5260688117)


def mat_mul(a, b):
    return [[sum(a[i][k] * b[k][j] for k in range(3)) for j in range(3)] for i in range(3)]


def mat_inv(m):
    a, b, c = m[0]
    d, e, f = m[1]
    g, h, i = m[2]
    det = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g)
    return [
        [(e * i - f * h) / det, (c * h - b * i) / det, (b * f - c * e) / det],
        [(f * g - d * i) / det, (a * i - c * g) / det, (c * d - a * f) / det],
        [(d * h - e * g) / det, (b * g - a * h) / det, (a * e - b * d) / det],
    ]


REC709_TO_EGAMUT = mat_mul(XYZ_TO_EGAMUT, mat_inv(XYZ_TO_REC709))

# (display, view, look, exposure, gamma): extra held-out cases besides every baked combination at
# exposure 0 / gamma 1. (Blender's `view_settings.look` enum only offers the looks of the current
# view, so a look the view ignores cannot be set and is not measured.)
EXTRA_CASES = [
    ("sRGB", "Standard", "None", -0.75, 1.6),
    ("sRGB", "Raw", "None", 0.5, 1.3),
    ("sRGB", "AgX", "None", 1.25, 0.8),
    ("sRGB", "AgX", "AgX - Punchy", -1.5, 1.2),
    ("sRGB", "Filmic", "High Contrast", 2.0, 1.0),
    ("sRGB", "ACES 2.0", "None", 0.0, 2.2),
    ("Display P3", "False Color", "False Color - Punchy", 0.5, 1.0),
]


# --- Blender's look rules and display transform -------------------------------------------------

LOOK_NAMES = [lk.getName() for lk in CONFIG.getLooks()]


def look_view(look):
    return look.split(" - ", 1)[0] if " - " in look else ""


def look_applies(view, look):
    """`colormanage_use_look` + `view_filter_for_look` (imbuf/intern/colormanagement.cc)."""
    if look == "None":
        return False
    if any(look_view(lk) == view for lk in LOOK_NAMES):
        flt = view
    else:
        short = view.split(" - ", 1)[0] if " - " in view else ""
        flt = short if short and any(look_view(lk) == short for lk in LOOK_NAMES) else ""
    return look_view(look) == flt


def look_result_space(look):
    """`LookTransform::GetLooksResultColorSpace`: the look's process space. A look whose
    transform is an empty group ("Medium Contrast") still converts into its process space and
    back (verified against `Image.save_render`), so it is not a no-op."""
    return CONFIG.getLook(look).getProcessSpace()


def blender_transform(display, view, look, src="Linear Rec.709"):
    """`create_ocio_display_transform` (libocio_display_processor.cc), look already filtered."""
    group = OCIO.GroupTransform()
    use_look = look != "None"
    if use_look:
        out = look_result_space(look)
        if out:
            lt = OCIO.LookTransform()
            lt.setSrc(src)
            view_cs = CONFIG.getDisplayViewColorSpaceName(display, view)
            cs = CONFIG.getColorSpace(view_cs) if view_cs else None
            if cs is not None and cs.isData():
                lt.setDst(src)
            else:
                lt.setDst(out)
                src = out
            lt.setLooks(look)
            group.appendTransform(lt)
        else:
            use_look = False
    dvt = OCIO.DisplayViewTransform()
    dvt.setSrc(src)
    dvt.setLooksBypass(use_look)
    dvt.setView(view)
    dvt.setDisplay(display)
    group.appendTransform(dvt)
    return group


def effective_look(view, look):
    """The look Blender actually applies ("None" when the view filters it out)."""
    return look if look_applies(view, look) else "None"


# --- LUT data -------------------------------------------------------------------------------------


def lut_key(t):
    if isinstance(t, OCIO.Lut3DTransform):
        n = t.getGridSize()
        vals = [t.getValue(r, g, b) for b in range(n) for g in range(n) for r in range(n)]
        return ("3d", n, tuple(tuple(v) for v in vals))
    n = t.getLength()
    vals = [tuple(t.getValue(i)) for i in range(n)]
    return ("1d", n, tuple(vals))


def data_name(stem):
    if stem == "AgX_Base_sRGB":
        return "agx"
    return stem.lower().replace("-", "_").replace(".", "_").replace("pbrneutral", "pbr_neutral")


def known_luts():
    """Every LUT file of the config's search path, loaded by Blender's OCIO: key -> name."""
    known = {}
    for sub in ("luts", "filmic"):
        folder = os.path.join(CONFIG_DIR, sub)
        for fname in sorted(os.listdir(folder)):
            stem, ext = os.path.splitext(fname)
            if ext not in (".cube", ".spi1d"):
                continue
            ft = OCIO.FileTransform(src=fname, interpolation=OCIO.INTERP_LINEAR)
            proc = CONFIG.getProcessor(ft).getOptimizedProcessor(OCIO.OPTIMIZATION_NONE)
            for t in proc.createGroupTransform():
                if isinstance(t, (OCIO.Lut1DTransform, OCIO.Lut3DTransform)):
                    known[lut_key(t)] = (data_name(stem), fname)
    return known


# 3D LUTs whose values are written with 5 decimals (below the 16-bit display step) to keep
# the files small; the others (the Rec.2020 base and the luminance compensation feed the
# banded False Color LUT, where 1e-5 moves a band edge) keep 8 significant digits.
COARSE_3D = {"agx", "agx_base_p3", "pbr_neutral", "filmic_desat_33"}


def fmt5(v):
    """5 decimals (below the 16-bit display step), trailing zeros stripped."""
    s = "%.5f" % v
    s = s.rstrip("0").rstrip(".")
    return "0" if s in ("", "-0") else s


def fmt8(v):
    s = "%.8g" % v
    return "0" if s in ("-0",) else s


def header_lines(title, source):
    build = bpy.app.build_hash
    build = build.decode() if isinstance(build, bytes) else build
    return [
        "TITLE \"frap %s\"" % title,
        "# Generated by tools/blender/bake_view_luts.py from Blender %s (%s); do not edit."
        % (bpy.app.version_string, build),
        "# Read from Blender's OpenColorIO processor (colormanagement LUT \"%s\"); "
        "see ARCHITECTURE.md 19.5." % source,
    ]


def write_lut(out_dir, name, key, source):
    kind, n, vals = key
    if kind == "3d":
        lines = header_lines(name, source)
        if name == "agx":
            # `lut::baked("AgX", "None", "sRGB")`: the base sRGB LUT is the whole AgX view on
            # the sRGB display (E-Gamut matrix, lg2 shaper, Rec.1886 values shown on sRGB).
            lines += [
                "# frap-view-transform: AgX",
                "# frap-look: None",
                "# frap-display: sRGB",
                "# frap-matrix: " + " ".join(repr(x) for row in REC709_TO_EGAMUT for x in row),
                "# frap-shaper: lg2 %r %r" % AGX_LG2,
                "# frap-interpolation: tetrahedral",
                "# frap-post: rec1886_to_srgb",
            ]
        lines += ["DOMAIN_MIN 0 0 0", "DOMAIN_MAX 1 1 1", "LUT_3D_SIZE %d" % n]
        fmt = fmt5 if name in COARSE_3D else fmt8
        lines += [" ".join(fmt(c) for c in v) for v in vals]
        path = os.path.join(out_dir, name + ".cube")
    else:
        single = all(v[0] == v[1] == v[2] for v in vals)
        lines = header_lines(name, source)
        lines += ["LUT_1D_SIZE %d" % n, "CHANNELS %d" % (1 if single else 3)]
        lines += [fmt8(v[0]) if single else " ".join(fmt8(c) for c in v) for v in vals]
        path = os.path.join(out_dir, name + ".lut1d")
    with open(path, "w", newline="\n") as f:
        f.write("\n".join(lines) + "\n")
    print("wrote", path)


# --- op serialisation -----------------------------------------------------------------------------


def num(v):
    return repr(float(v))


def direction(t):
    return "fwd" if t.getDirection() == OCIO.TRANSFORM_DIR_FORWARD else "inv"


def rgbm(v):
    return [v.red, v.green, v.blue, v.master]


def serialise(t, luts, known):
    """One OCIO transform of an optimised processor -> one `.ops` line."""
    if isinstance(t, OCIO.MatrixTransform):
        m = t.getMatrix()
        o = t.getOffset()
        assert m[3] == m[7] == m[11] == m[12] == m[13] == m[14] == 0.0 and m[15] == 1.0, m
        assert o[3] == 0.0 and t.getDirection() == OCIO.TRANSFORM_DIR_FORWARD
        vals = [m[0], m[1], m[2], m[4], m[5], m[6], m[8], m[9], m[10], o[0], o[1], o[2]]
        return "matrix " + " ".join(num(x) for x in vals)
    if isinstance(t, OCIO.LogTransform):
        return "log %s %s" % (num(t.getBase()), direction(t))
    if isinstance(t, (OCIO.Lut1DTransform, OCIO.Lut3DTransform)):
        key = lut_key(t)
        name, source = known[key]
        luts[name] = (key, source)
        if isinstance(t, OCIO.Lut3DTransform):
            assert t.getInterpolation() == OCIO.INTERP_TETRAHEDRAL
            assert t.getDirection() == OCIO.TRANSFORM_DIR_FORWARD
            return "lut3d %s" % name
        assert t.getInterpolation() in (OCIO.INTERP_LINEAR, OCIO.INTERP_DEFAULT)
        assert not t.getInputHalfDomain() and t.getHueAdjust() == OCIO.HUE_NONE
        return "lut1d %s %s" % (name, direction(t))
    if isinstance(t, OCIO.ExponentTransform):
        v = t.getValue()
        assert v[3] == 1.0
        style = {OCIO.NEGATIVE_CLAMP: "clamp", OCIO.NEGATIVE_MIRROR: "mirror",
                 OCIO.NEGATIVE_PASS_THRU: "pass_thru"}[t.getNegativeStyle()]
        return "exponent %s %s %s" % (" ".join(num(x) for x in v[:3]), style, direction(t))
    if isinstance(t, OCIO.ExponentWithLinearTransform):
        g = t.getGamma()
        o = t.getOffset()
        style = {OCIO.NEGATIVE_LINEAR: "linear", OCIO.NEGATIVE_MIRROR: "mirror"}[
            t.getNegativeStyle()]
        return "moncurve %s %s %s %s" % (" ".join(num(x) for x in g[:3]),
                                         " ".join(num(x) for x in o[:3]), style, direction(t))
    if isinstance(t, OCIO.RangeTransform):
        assert t.getStyle() == OCIO.RANGE_CLAMP and t.getDirection() == OCIO.TRANSFORM_DIR_FORWARD
        vals = [
            num(t.getMinInValue()) if t.hasMinInValue() else "-",
            num(t.getMaxInValue()) if t.hasMaxInValue() else "-",
            num(t.getMinOutValue()) if t.hasMinOutValue() else "-",
            num(t.getMaxOutValue()) if t.hasMaxOutValue() else "-",
        ]
        return "range " + " ".join(vals)
    if isinstance(t, OCIO.CDLTransform):
        style = "noclamp" if t.getStyle() == OCIO.CDL_NO_CLAMP else "clamp"
        vals = list(t.getSlope()) + list(t.getOffset()) + list(t.getPower()) + [t.getSat()]
        return "cdl %s %s %s" % (" ".join(num(x) for x in vals), style, direction(t))
    if isinstance(t, OCIO.GradingPrimaryTransform):
        assert t.getStyle() == OCIO.GRADING_LOG
        v = t.getValue()
        vals = (rgbm(v.brightness) + rgbm(v.contrast) + rgbm(v.gamma) +
                [v.saturation, v.pivot, v.pivotBlack, v.pivotWhite, v.clampBlack, v.clampWhite])
        return "primary_log %s %s" % (" ".join(num(x) for x in vals), direction(t))
    if isinstance(t, OCIO.GradingToneTransform):
        assert t.getStyle() == OCIO.GRADING_LOG
        v = t.getValue()
        vals = []
        for z in (v.blacks, v.shadows, v.midtones, v.highlights, v.whites):
            vals += [z.red, z.green, z.blue, z.master, z.start, z.width]
        vals.append(v.scontrast)
        return "tone_log %s %s" % (" ".join(num(x) for x in vals), direction(t))
    if isinstance(t, OCIO.GradingRGBCurveTransform):
        assert t.getStyle() == OCIO.GRADING_LOG and not t.getBypassLinToLog()
        v = t.getValue()
        parts = []
        for c in (v.red, v.green, v.blue, v.master):
            assert c.getSplineType() == OCIO.B_SPLINE
            cps = list(c.getControlPoints())
            slopes = list(c.getSlopes())
            if len(slopes) != len(cps):
                slopes = [0.0] * len(cps)
            parts.append(str(len(cps)))
            for cp, s in zip(cps, slopes):
                parts += [num(cp.x), num(cp.y), num(s)]
        return "curve_log %s %s" % (" ".join(parts), direction(t))
    if isinstance(t, OCIO.FixedFunctionTransform):
        style = {
            OCIO.FIXED_FUNCTION_ACES_GLOW_10: "aces_glow_10",
            OCIO.FIXED_FUNCTION_ACES_RED_MOD_10: "aces_red_mod_10",
            OCIO.FIXED_FUNCTION_ACES_DARK_TO_DIM_10: "aces_dark_to_dim_10",
            OCIO.FIXED_FUNCTION_ACES_GAMUT_COMP_13: "aces_gamut_comp_13",
            OCIO.FIXED_FUNCTION_ACES_OUTPUT_TRANSFORM_20: "aces_output_transform_20",
        }[t.getStyle()]
        params = " ".join(num(x) for x in t.getParams())
        return ("fixed %s %s %s" % (style, direction(t), params)).rstrip()
    raise ValueError("unsupported OCIO transform %r" % t)


def pipeline_ops(display, view, look, luts, known):
    proc = CONFIG.getProcessor(blender_transform(display, view, look))
    proc = proc.getOptimizedProcessor(OCIO.OPTIMIZATION_LOSSLESS)
    return [serialise(t, luts, known) for t in proc.createGroupTransform()]


def combinations():
    """(display, view, look) for every baked pipeline, in a stable order."""
    out = []
    for display in DISPLAYS:
        for view in CONFIG.getViews(display):
            if view not in FAMILY:
                continue
            for look in ["None"] + LOOK_NAMES:
                if look == "None" or effective_look(view, look) == look:
                    out.append((display, view, look))
    return out


def bake_pipelines(out_dir):
    known = known_luts()
    luts = {}
    files = {}
    for display, view, look in combinations():
        ops = pipeline_ops(display, view, look, luts, known)
        files.setdefault(FAMILY[view], []).append((display, view, look, ops))
    for family, sections in sorted(files.items()):
        build = bpy.app.build_hash
        build = build.decode() if isinstance(build, bytes) else build
        lines = [
            "# frap OCIO op pipelines (%s): Blender %s (%s) display transforms, generated by"
            % (family, bpy.app.version_string, build),
            "# tools/blender/bake_view_luts.py; do not edit. One [display|view|look] section per",
            "# combination, one op per line (crates/frap-color/src/display/ocio.rs).",
        ]
        for display, view, look, ops in sections:
            lines.append("")
            lines.append("[%s|%s|%s]" % (display, view, look))
            lines += ops
        path = os.path.join(out_dir, family + ".ops")
        with open(path, "w", newline="\n") as f:
            f.write("\n".join(lines) + "\n")
        print("wrote", path)
    for name, entry in sorted(luts.items()):
        write_lut(out_dir, name, entry[0], entry[1])


# --- PNG (8/16-bit RGB) reader ----------------------------------------------------------------


def read_png_rgb(path):
    with open(path, "rb") as f:
        data = f.read()
    assert data[:8] == b"\x89PNG\r\n\x1a\n", "not a PNG"
    pos = 8
    idat = bytearray()
    width = height = None
    while pos < len(data):
        (length,) = struct.unpack(">I", data[pos:pos + 4])
        ctype = data[pos + 4:pos + 8]
        chunk = data[pos + 8:pos + 8 + length]
        pos += 12 + length
        if ctype == b"IHDR":
            width, height, depth, color_type, _c, _f, interlace = struct.unpack(">IIBBBBB", chunk)
            assert depth in (8, 16) and color_type == 2 and interlace == 0, (
                depth, color_type, interlace)
        elif ctype == b"IDAT":
            idat += chunk
        elif ctype == b"IEND":
            break
    raw = zlib.decompress(bytes(idat))
    bpp = 3 * depth // 8
    stride = width * bpp
    prev = bytearray(stride)
    rows = []
    p = 0
    for _ in range(height):
        ftype = raw[p]
        line = bytearray(raw[p + 1:p + 1 + stride])
        p += 1 + stride
        if ftype == 1:
            for i in range(bpp, stride):
                line[i] = (line[i] + line[i - bpp]) & 0xFF
        elif ftype == 2:
            for i in range(stride):
                line[i] = (line[i] + prev[i]) & 0xFF
        elif ftype == 3:
            for i in range(stride):
                left = line[i - bpp] if i >= bpp else 0
                line[i] = (line[i] + ((left + prev[i]) >> 1)) & 0xFF
        elif ftype == 4:
            for i in range(stride):
                a = line[i - bpp] if i >= bpp else 0
                b = prev[i]
                c = prev[i - bpp] if i >= bpp else 0
                pa = abs(b - c)
                pb = abs(a - c)
                pc = abs(a + b - 2 * c)
                pred = a if (pa <= pb and pa <= pc) else (b if pb <= pc else c)
                line[i] = (line[i] + pred) & 0xFF
        else:
            assert ftype == 0, ftype
        rows.append(line)
        prev = line
    # Return pixels in Blender order (bottom row first) as integer triples.
    out = []
    for line in reversed(rows):
        vals = struct.unpack(">%d%s" % (width * 3, "H" if depth == 16 else "B"), bytes(line))
        out.extend(vals[i:i + 3] for i in range(0, len(vals), 3))
    return width, height, out


# --- rendering through save_render ------------------------------------------------------------


def f32(x):
    return struct.unpack("<f", struct.pack("<f", x))[0]


def render_colors(colors, view_transform, look, display_device, exposure, gamma, tmpdir,
                  depth=16, dither=0.0, width=None):
    """Pushes scene-linear Rec.709 colours (Blender pixel order: bottom row first) through
    save_render; returns `depth`-bit display triples in the same order."""
    n = len(colors)
    width = width or min(n, 4096)
    height = (n + width - 1) // width
    pixels = [0.0] * (width * height * 4)
    for i, c in enumerate(colors):
        pixels[4 * i:4 * i + 4] = [c[0], c[1], c[2], 1.0]
    for i in range(n, width * height):
        pixels[4 * i + 3] = 1.0
    img = bpy.data.images.new("frap_bake", width, height, alpha=True, float_buffer=True)
    img.colorspace_settings.name = "Linear Rec.709"
    img.pixels.foreach_set(pixels)
    img.update()

    scene = bpy.context.scene
    scene.display_settings.display_device = display_device
    scene.view_settings.view_transform = view_transform
    scene.view_settings.look = look
    scene.view_settings.exposure = exposure
    scene.view_settings.gamma = gamma
    scene.view_settings.use_curve_mapping = False
    scene.view_settings.use_white_balance = False
    scene.render.dither_intensity = dither
    assert (scene.display_settings.display_device, scene.view_settings.view_transform,
            scene.view_settings.look) == (display_device, view_transform, look)
    fmt = scene.render.image_settings
    fmt.file_format = "PNG"
    fmt.color_mode = "RGB"
    fmt.color_depth = str(depth)
    fmt.compression = 0

    path = os.path.join(tmpdir, "bake.png")
    img.save_render(path, scene=scene)
    bpy.data.images.remove(img)
    w, h, out = read_png_rgb(path)
    assert (w, h) == (width, height)
    os.remove(path)
    return out[:n]


def heldout_inputs(count, seed):
    rng = random.Random(seed)
    colors = []
    for i in range(count):
        kind = i % 10
        if kind < 6:  # independent log-uniform channels over the HDR range
            c = [2.0 ** rng.uniform(-13.0, 10.0) for _ in range(3)]
        elif kind < 7:  # neutrals
            v = 2.0 ** rng.uniform(-13.0, 10.0)
            c = [v, v, v]
        elif kind < 9:  # saturated: one or two channels zero
            c = [2.0 ** rng.uniform(-8.0, 6.0) for _ in range(3)]
            z = rng.randrange(3)
            c[z] = 0.0
            if rng.random() < 0.5:
                c[(z + 1) % 3] = 0.0
        else:  # ordinary [0, 1] albedo-like values
            c = [rng.random() for _ in range(3)]
        colors.append([f32(x) for x in c])
    return colors


def bake_dither(tmpdir):
    """Flat Workbench render saved dithered (8-bit) and undithered (16-bit). Changes the scene,
    so it runs last."""
    scene = bpy.context.scene
    for ob in list(bpy.data.objects):
        if ob.type != "CAMERA":
            bpy.data.objects.remove(ob)
    width, height = 37, 23
    scene.render.engine = "BLENDER_WORKBENCH"
    scene.render.resolution_x = width
    scene.render.resolution_y = height
    scene.render.resolution_percentage = 100
    scene.world.color = (0.18, 0.18, 0.18)
    scene.display_settings.display_device = "sRGB"
    scene.view_settings.view_transform = "Standard"
    scene.view_settings.look = "None"
    scene.view_settings.exposure = 0.0
    scene.view_settings.gamma = 1.0
    scene.render.dither_intensity = 1.0
    fmt = scene.render.image_settings
    fmt.file_format = "PNG"
    fmt.color_mode = "RGB"
    fmt.compression = 0
    result = {}
    for depth in (8, 16):
        fmt.color_depth = str(depth)
        path = os.path.join(tmpdir, "dither%d.png" % depth)
        scene.render.filepath = path
        bpy.ops.render.render(write_still=True)
        w, h, out = read_png_rgb(path)
        assert (w, h) == (width, height)
        os.remove(path)
        result[depth] = [v[0] for v in out]  # grey: red channel
    return {
        "note": "Flat Workbench render (world colour 0.18, Standard), render.dither_intensity 1. "
                "Red channel, bottom row first as Blender stores it: display_u16 from a 16-bit "
                "save (undithered), outputs_u8 from an 8-bit save (dithered).",
        "width": width,
        "height": height,
        "intensity": 1.0,
        "display_u16": result[16],
        "outputs_u8": result[8],
    }


def main():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    out_dir = "assets/luts"
    count = 400
    seed = 20261116
    i = 0
    while i < len(argv):
        if argv[i] == "--out":
            out_dir = argv[i + 1]
        elif argv[i] == "--heldout":
            count = int(argv[i + 1])
        elif argv[i] == "--seed":
            seed = int(argv[i + 1])
        else:
            raise SystemExit("unknown argument " + argv[i])
        i += 2
    os.makedirs(out_dir, exist_ok=True)
    tmpdir = tempfile.mkdtemp(prefix="frap_bake_")
    bake_pipelines(out_dir)

    inputs = heldout_inputs(count, seed)
    cases = []
    settings = [(d, v, lk, 0.0, 1.0) for d, v, lk in combinations()]
    settings += EXTRA_CASES
    for display, view, look, exposure, gamma in settings:
        out = render_colors(inputs, view, look, display, exposure, gamma, tmpdir)
        cases.append({
            "view_transform": view,
            "look": look,
            "display_device": display,
            "exposure": exposure,
            "gamma": gamma,
            "outputs_u16": [list(v) for v in out],
        })
    dither = bake_dither(tmpdir)

    doc = {
        "generator": "tools/blender/bake_view_luts.py",
        "blender": bpy.app.version_string,
        "seed": seed,
        "note": "Held-out colours (scene-linear Rec.709, f32-exact) and Blender's 16-bit display "
                "outputs through Image.save_render (dither 0), per display/view/look/exposure/"
                "gamma.",
        "inputs": inputs,
        "cases": cases,
        "dither": dither,
    }
    path = os.path.join(out_dir, "heldout.json")
    with open(path, "w", newline="\n") as f:
        json.dump(doc, f, indent=None, separators=(",", ":"))
        f.write("\n")
    print("wrote", path, "(%d cases)" % len(cases))
    os.rmdir(tmpdir)


main()
