# SPDX-License-Identifier: GPL-3.0-or-later
"""Bake Blender's view transforms into `.cube` LUTs and a held-out colour set (ARCHITECTURE §19.5).

Runs INSIDE Blender:

    $BLENDER_BIN -b --factory-startup --python tools/blender/bake_view_luts.py -- \
        --out assets/luts [--only AgX] [--heldout 400] [--seed 20261116] [--size N]

Method (no Blender LUT file is copied; everything is measured through `Image.save_render`):

1. For every entry of LUT_SPECS, build a float image whose pixels are the lattice points of the
   LUT, expressed in scene-linear Rec.709. A lattice point (i, j, k) of size N is the shaper
   input s = (i, j, k) / (N - 1); its scene-linear value is `inv(matrix) @ 2^(lo + s*(hi - lo))`
   (the spec's log2 shaper and optional 3x3 input matrix; the shaper of a view transform is chosen
   to be the one Blender's OCIO config uses, so the baked lattice lands on Blender's own LUT
   nodes and tetrahedral interpolation between them reproduces Blender, see `fit` below).
2. Save it with `image.save_render()` as a 16-bit RGB PNG with the scene's `view_settings`
   (`view_transform`, `look`, exposure 0, gamma 1) and `display_settings.display_device`,
   `render.dither_intensity = 0`, and decode the PNG back here (pure-Python PNG reader).
3. Fit: the display-encoded 16-bit outputs are mapped through the inverse of the spec's `post`
   transfer (e.g. AgX's LUT is Rec.1886-encoded and Blender converts it to the sRGB display after
   interpolation), giving the LUT node values, written as `assets/luts/<slug>.cube` with the
   shaper/matrix/post parameters in `# frap-*:` header comments.
4. Held-out set: random HDR colours (fixed seed, never on the lattice) are rendered the same way
   for every entry of HELDOUT_CASES (incl. Standard and Raw, which frap computes analytically,
   and non-default exposure/gamma) and written with Blender's 16-bit outputs to
   `assets/luts/heldout.json`. frap's tests measure accuracy on these colours only, never on the
   lattice the LUT was fitted to.
5. Dither: `Image.save_render` does not dither (only render results carry
   `render.dither_intensity`), so a flat Workbench render (world colour only) is written twice
   with `render.render(write_still=True)`: as an 8-bit PNG with `dither_intensity = 1` and as a
   16-bit PNG (undithered display values). This pins Blender's noise pattern and amplitude.

The OCIO parameters quoted below come from Blender 5.2.4's
`datafiles/colormanagement/config.ocio` (provenance comments at each constant).
"""

import json
import os
import random
import struct
import sys
import tempfile
import zlib

import bpy

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


def mat_vec(m, v):
    return [m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2] for i in range(3)]


REC709_TO_EGAMUT = mat_mul(XYZ_TO_EGAMUT, mat_inv(XYZ_TO_REC709))

# One entry per baked LUT. `post` names the per-channel transfer applied after the LUT:
#   "rec1886_to_srgb": out = srgb_encode(max(v, 0) ^ 2.4)  (AgX Base Rec.1886 on the sRGB display)
#   "none":            out = v
# `size` 57 puts every lattice point on a node of Blender's own 57^3 AgX LUT, so tetrahedral
# interpolation between baked nodes is Blender's interpolation (a 29^3 lattice on every second
# node measured CIEDE2000 up to 2.0 on saturated held-out colours; 57^3 measures < 0.01).
LUT_SPECS = [
    {
        "view_transform": "AgX",
        "look": "None",
        "display_device": "sRGB",
        "matrix": REC709_TO_EGAMUT,
        "shaper": AGX_LG2,
        "size": 57,
        "post": "rec1886_to_srgb",
    },
]

# (view_transform, look, exposure, gamma) on the sRGB display.
HELDOUT_CASES = [
    ("Standard", "None", 0.0, 1.0),
    ("Standard", "None", -0.75, 1.6),
    ("Raw", "None", 0.0, 1.0),
    ("Raw", "None", 0.5, 1.3),
    ("AgX", "None", 0.0, 1.0),
    ("AgX", "None", 1.25, 0.8),
]


def slug(spec):
    s = spec["view_transform"].lower().replace(" ", "_").replace(".", "")
    if spec["look"] != "None":
        s += "__" + spec["look"].lower().replace(" ", "_").replace(".", "")
    if spec["display_device"] != "sRGB":
        s += "@" + spec["display_device"].lower().replace(" ", "_").replace(".", "")
    return s


# --- OCIO ExponentWithLinearTransform {gamma: 2.4, offset: 0.055} (the sRGB display) ---------
SRGB_G = 2.4
SRGB_O = 0.055
SRGB_EBREAK = SRGB_O / (SRGB_G - 1.0)
SRGB_LBREAK = ((SRGB_EBREAK + SRGB_O) / (1.0 + SRGB_O)) ** SRGB_G
SRGB_SLOPE = SRGB_EBREAK / SRGB_LBREAK


def srgb_decode(e):
    if e < SRGB_EBREAK:
        return e / SRGB_SLOPE
    return ((e + SRGB_O) / (1.0 + SRGB_O)) ** SRGB_G


def post_inverse(post, v):
    if post == "none":
        return v
    if post == "rec1886_to_srgb":
        lin = srgb_decode(v)
        return max(lin, 0.0) ** (1.0 / 2.4)
    raise ValueError(post)


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


def lattice_inputs(spec):
    n = spec["size"]
    lo, hi = spec["shaper"]
    inv = mat_inv(spec["matrix"]) if spec["matrix"] else None
    colors = []
    for b in range(n):
        for g in range(n):
            for r in range(n):
                shaped = [2.0 ** (lo + (k / (n - 1)) * (hi - lo)) for k in (r, g, b)]
                lin = mat_vec(inv, shaped) if inv else shaped
                colors.append([f32(x) for x in lin])
    return colors


def fmt5(v):
    """5 decimals (below the 16-bit measurement step), trailing zeros stripped."""
    s = "%.5f" % v
    s = s.rstrip("0").rstrip(".")
    return "0" if s in ("", "-0") else s


def bake_lut(spec, out_dir, tmpdir):
    colors = lattice_inputs(spec)
    out = render_colors(colors, spec["view_transform"], spec["look"], spec["display_device"],
                        0.0, 1.0, tmpdir)
    path = os.path.join(out_dir, slug(spec) + ".cube")
    lines = [
        'TITLE "frap %s"' % slug(spec),
        "# Generated by tools/blender/bake_view_luts.py from Blender %s (%s); do not edit."
        % (bpy.app.version_string, bpy.app.build_hash.decode() if isinstance(bpy.app.build_hash, bytes)
           else bpy.app.build_hash),
        "# Measured through Image.save_render (16-bit PNG, dither 0); see ARCHITECTURE.md 19.5.",
        "# frap-view-transform: %s" % spec["view_transform"],
        "# frap-look: %s" % spec["look"],
        "# frap-display: %s" % spec["display_device"],
    ]
    if spec["matrix"]:
        lines.append("# frap-matrix: " + " ".join(repr(x) for row in spec["matrix"] for x in row))
    lines.append("# frap-shaper: lg2 %r %r" % tuple(spec["shaper"]))
    lines.append("# frap-interpolation: tetrahedral")
    lines.append("# frap-post: %s" % spec["post"])
    lines.append("DOMAIN_MIN 0 0 0")
    lines.append("DOMAIN_MAX 1 1 1")
    lines.append("LUT_3D_SIZE %d" % spec["size"])
    for rgb in out:
        vals = [post_inverse(spec["post"], v / 65535.0) for v in rgb]
        lines.append(" ".join(fmt5(v) for v in vals))
    with open(path, "w", newline="\n") as f:
        f.write("\n".join(lines) + "\n")
    print("wrote", path)


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
    only = None
    count = 400
    seed = 20261116
    i = 0
    while i < len(argv):
        if argv[i] == "--out":
            out_dir = argv[i + 1]
        elif argv[i] == "--only":
            only = argv[i + 1].split(",")
        elif argv[i] == "--heldout":
            count = int(argv[i + 1])
        elif argv[i] == "--seed":
            seed = int(argv[i + 1])
        elif argv[i] == "--size":
            for spec in LUT_SPECS:
                spec["size"] = int(argv[i + 1])
        else:
            raise SystemExit("unknown argument " + argv[i])
        i += 2
    os.makedirs(out_dir, exist_ok=True)
    tmpdir = tempfile.mkdtemp(prefix="frap_bake_")
    for spec in LUT_SPECS:
        if only is None or spec["view_transform"] in only:
            bake_lut(spec, out_dir, tmpdir)

    inputs = heldout_inputs(count, seed)
    cases = []
    for view, look, exposure, gamma in HELDOUT_CASES:
        out = render_colors(inputs, view, look, "sRGB", exposure, gamma, tmpdir)
        cases.append({
            "view_transform": view,
            "look": look,
            "display_device": "sRGB",
            "exposure": exposure,
            "gamma": gamma,
            "outputs_u16": [list(v) for v in out],
        })
    dither = bake_dither(tmpdir)

    doc = {
        "generator": "tools/blender/bake_view_luts.py",
        "blender": bpy.app.version_string,
        "seed": seed,
        "note": "Held-out colours (scene-linear Rec.709, f32-exact) and Blender's 16-bit sRGB "
                "display outputs through Image.save_render (dither 0). Never on a LUT lattice.",
        "inputs": inputs,
        "cases": cases,
        "dither": dither,
    }
    path = os.path.join(out_dir, "heldout.json")
    with open(path, "w", newline="\n") as f:
        json.dump(doc, f, indent=None, separators=(",", ":"))
        f.write("\n")
    print("wrote", path)
    os.rmdir(tmpdir)


main()
