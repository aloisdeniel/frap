# SPDX-License-Identifier: GPL-3.0-or-later
"""Write small images with Blender's own writers, for frap-imbuf's interop tests (WP 1.6).

    $BLENDER_BIN -b --factory-startup --python crates/frap-imbuf/tests/data/make_blender_fixtures.py -- \
        crates/frap-imbuf/tests/data/blender

Every file holds the same 5x3 RGBA pattern (`expected.json`, top row first, as frap stores
images). Images use the Non-Color colour space so Blender writes the samples unconverted.
Byte formats get k/255 values; float formats get HDR floats exactly representable in half.
"""

import json
import os
import sys

import bpy

W, H = 5, 3


def pattern_byte():
    px = []
    for i in range(W * H):
        px.append([(i * 17) % 256, (i * 53 + 40) % 256, (255 - i * 11) % 256, 255 if i % 4 else 128])
    return px


def pattern_float():
    px = []
    for i in range(W * H):
        px.append([i * 0.25, 1.0 / (i + 1), 2.0 ** (i % 7 - 3), 1.0 if i % 4 else 0.5])
    return px


def to_blender_order(px):
    # Blender stores the bottom row first.
    rows = [px[r * W:(r + 1) * W] for r in range(H)]
    return [p for row in reversed(rows) for p in row]


def save(name, pixels, float_buffer, fmt, depth=None, quality=None, codec=None, mode="RGBA"):
    img = bpy.data.images.new(name, W, H, alpha=True, float_buffer=float_buffer)
    img.colorspace_settings.name = "Non-Color"
    flat = []
    for p in to_blender_order(pixels):
        flat.extend(p)
    img.pixels.foreach_set(flat)
    img.update()
    scene = bpy.context.scene
    s = scene.render.image_settings
    s.file_format = fmt
    s.color_mode = mode
    if depth:
        s.color_depth = depth
    if quality is not None:
        s.quality = quality
    if codec:
        s.exr_codec = codec
    s.color_management = "OVERRIDE"
    s.linear_colorspace_settings.name = "Non-Color"
    path = os.path.join(OUT, name)
    img.save_render(path, scene=scene)
    bpy.data.images.remove(img)
    return path


argv = sys.argv[sys.argv.index("--") + 1:]
OUT = argv[0]
os.makedirs(OUT, exist_ok=True)
byte_px = [[c / 255.0 for c in p] for p in pattern_byte()]
float_px = pattern_float()

save("png8.png", byte_px, False, "PNG", depth="8")
save("png16.png", byte_px, True, "PNG", depth="16")
save("tga.tga", byte_px, False, "TARGA")
save("bmp.bmp", byte_px, False, "BMP", mode="RGB")
save("jpeg.jpg", byte_px, False, "JPEG", quality=100, mode="RGB")
save("webp.webp", byte_px, False, "WEBP", quality=100)
save("exr_half.exr", float_px, True, "OPEN_EXR", depth="16", codec="ZIP")
save("exr_float.exr", float_px, True, "OPEN_EXR", depth="32", codec="ZIP")
save("hdr.hdr", float_px, True, "HDR", mode="RGB")

with open(os.path.join(OUT, "expected.json"), "w", newline="\n") as f:
    json.dump({
        "generator": "crates/frap-imbuf/tests/data/make_blender_fixtures.py",
        "blender": bpy.app.version_string,
        "width": W,
        "height": H,
        "byte_rgba": pattern_byte(),
        "float_rgba": float_px,
    }, f, indent=1)
    f.write("\n")
