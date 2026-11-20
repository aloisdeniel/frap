# SPDX-License-Identifier: GPL-3.0-or-later
"""Blender 5.2.4's render outputs of one film in every format frap writes (WP 5.4).

    $BLENDER_BIN -b --factory-startup --python \\
        tests/golden/fixtures/render/output/film/generate.py -- tests/golden/fixtures/render/output/film

Renders a small EEVEE scene (emissive planes from 0.01 to 10 W/m^2-equivalent strengths, so the
film has dark, saturated, over-bright and anti-aliased partially transparent pixels) with
`bpy.ops.render.render(write_still=True)` once per output setting, plus the film itself as a
32-bit OpenEXR (Blender's premultiplied, scene-linear render buffer). `manifest.json` lists every
file with its `image_settings`, `view_settings` and `film_transparent`.

frap's tests feed the same film to the output code (`frap-ops-render` `output::format`, the
encoders of `frap-imbuf`) and compare with these files: no GPU and no renderer is involved, so
the comparison is exact (bytes for TGA, BMP, HDR and JPEG; samples for the others). Renders of
one scene are deterministic: a second OpenEXR render of the film is checked to hold the same
samples as `film_transparent.exr`, then deleted.

`noise.png` and `noise_q*.jpg` are a 75 x 43 noise image saved with `Image.save_render` as PNG
and as JPEG from the same byte buffer: an odd size, so the JPEG writer pads with replicated
edges and dummy blocks.
"""

import json
import os
import random
import sys

import bpy
import numpy as np
import OpenImageIO as oiio

OUT = sys.argv[sys.argv.index("--") + 1]
os.makedirs(OUT, exist_ok=True)
W, H = 64, 48

scene = bpy.context.scene
bpy.data.objects.remove(bpy.data.objects["Cube"])
scene.render.engine = "BLENDER_EEVEE"
scene.render.resolution_x = W
scene.render.resolution_y = H
scene.render.resolution_percentage = 100
scene.world.color = (0.0, 0.0, 0.0)
camera = bpy.data.objects["Camera"]
camera.location = (0.0, 0.0, 4.0)
camera.rotation_euler = (0.0, 0.0, 0.0)

COLOURS = [
    (1.0, 0.2, 0.05, 1.0),
    (0.1, 0.8, 0.3, 1.0),
    (0.2, 0.3, 1.0, 1.0),
    (1.0, 1.0, 1.0, 1.0),
    (0.9, 0.9, 0.1, 1.0),
    (0.5, 0.1, 0.9, 1.0),
]
for i, strength in enumerate([0.01, 0.1, 0.5, 1.0, 3.0, 10.0]):
    bpy.ops.mesh.primitive_plane_add(size=0.9, location=((i % 3) - 1.0, (i // 3) - 0.5, 0.0))
    ob = bpy.context.object
    ob.rotation_euler = (0.3 * i, 0.2, 0.4 * i)
    mat = bpy.data.materials.new(f"E{i}")
    tree = mat.node_tree
    for node in list(tree.nodes):
        if node.type != "OUTPUT_MATERIAL":
            tree.nodes.remove(node)
    emission = tree.nodes.new("ShaderNodeEmission")
    emission.inputs["Color"].default_value = COLOURS[i]
    emission.inputs["Strength"].default_value = strength
    tree.links.new(emission.outputs[0], tree.nodes["Material Output"].inputs[0])
    ob.data.materials.append(mat)

DEFAULTS = {
    "file_format": "PNG",
    "color_mode": "RGBA",
    "color_depth": "8",
    "quality": 90,
    "compression": 15,
    "exr_codec": "ZIP",
    "tiff_codec": "DEFLATE",
    "use_cineon_log": False,
}

manifest = []


def render(name, transparent=True, view="Standard", dither=1.0, **settings):
    """Renders with `settings` over the defaults; returns Blender's error message, if any."""
    scene.render.film_transparent = transparent
    scene.render.dither_intensity = dither
    scene.view_settings.view_transform = view
    s = scene.render.image_settings
    merged = dict(DEFAULTS, **settings)
    # media_type, then file_format: setting it adjusts color_mode and color_depth.
    s.media_type = "MULTI_LAYER_IMAGE" if merged["file_format"] == "OPEN_EXR_MULTILAYER" else "IMAGE"
    s.file_format = merged["file_format"]
    for key in ("color_mode", "color_depth", "quality", "compression", "exr_codec", "tiff_codec",
                "use_cineon_log"):
        setattr(s, key, merged[key])
    scene.render.filepath = os.path.join(OUT, name)
    entry = {
        "name": name,
        "film": "film_transparent.exr" if transparent else "film_opaque.exr",
        "view_transform": view,
        "dither_intensity": dither,
        "image_settings": dict({k: getattr(s, k) for k in DEFAULTS}, media_type=s.media_type),
    }
    try:
        bpy.ops.render.render(write_still=True)
    except RuntimeError as e:
        # The message names the output path: keep it relative to the fixture directory.
        entry["error"] = str(e).strip().replace(os.path.join(OUT, ""), "")
    else:
        entry["file"] = name + scene.render.file_extension
    manifest.append(entry)


# The films (and a determinism check: a second copy must be identical).
render("film_transparent", file_format="OPEN_EXR", color_depth="32", exr_codec="ZIP")
render("film_opaque", transparent=False, file_format="OPEN_EXR", color_depth="32", exr_codec="ZIP")
render("check_transparent", file_format="OPEN_EXR", color_depth="32", exr_codec="ZIP")

render("png8_rgba")
render("png8_rgb", color_mode="RGB")
render("png8_bw", color_mode="BW")
render("png8_rgba_nodither", dither=0.0)
render("png8_rgba_opaque", transparent=False)
render("png8_rgba_agx", view="AgX")
render("png16_rgba", color_depth="16")
render("png16_rgb", color_mode="RGB", color_depth="16")
render("png16_bw", color_mode="BW", color_depth="16")
render("png16_rgba_agx", color_depth="16", view="AgX")
# Raw view: no transform, so frap's output path is compared without its sRGB curve.
render("png8_rgba_raw", view="Raw")
render("png16_rgba_raw", color_depth="16", view="Raw")
render("png16_rgb_raw", color_mode="RGB", color_depth="16", view="Raw")
render("png16_bw_raw", color_mode="BW", color_depth="16", view="Raw")
render("jpeg_rgb", file_format="JPEG", color_mode="RGB")
render("jpeg_bw", file_format="JPEG", color_mode="BW")
render("jpeg_rgb_q5", file_format="JPEG", color_mode="RGB", quality=5)
render("jpeg_rgb_q100", file_format="JPEG", color_mode="RGB", quality=100)
render("webp_rgba", file_format="WEBP", quality=100)
render("webp_rgb", file_format="WEBP", color_mode="RGB", quality=100)
render("webp_bw", file_format="WEBP", color_mode="BW", quality=100)
render("webp_rgba_q80", file_format="WEBP", quality=80)
render("exr16_rgba_zip", file_format="OPEN_EXR", color_depth="16")
render("exr32_rgb_none", file_format="OPEN_EXR", color_mode="RGB", color_depth="32", exr_codec="NONE")
render("exr32_bw_piz", file_format="OPEN_EXR", color_mode="BW", color_depth="32", exr_codec="PIZ")
render("exr16_bw_rle", file_format="OPEN_EXR", color_mode="BW", color_depth="16", exr_codec="RLE")
render("exr32_rgba_zips", file_format="OPEN_EXR", color_depth="32", exr_codec="ZIPS")
render("exr32_rgba_pxr24", file_format="OPEN_EXR", color_depth="32", exr_codec="PXR24")
render("exr16_rgba_b44", file_format="OPEN_EXR", color_depth="16", exr_codec="B44")
render("exr16_rgba_dwaa", file_format="OPEN_EXR", color_depth="16", exr_codec="DWAA")
render("hdr_rgb", file_format="HDR", color_mode="RGB", color_depth="32")
render("hdr_bw", file_format="HDR", color_mode="BW", color_depth="32")
render("tga_rgba", file_format="TARGA")
render("tga_rgb", file_format="TARGA", color_mode="RGB")
render("tga_bw", file_format="TARGA", color_mode="BW")
render("tgaraw_rgba", file_format="TARGA_RAW")
render("tgaraw_bw", file_format="TARGA_RAW", color_mode="BW")
render("bmp_rgb", file_format="BMP", color_mode="RGB")
render("bmp_bw", file_format="BMP", color_mode="BW")
render("bmp_rgba", file_format="BMP", color_mode="RGBA")
# M5+M6 stabilization: TIFF, IRIS, DPX, Cineon and multilayer OpenEXR.
render("tiff8_rgba", file_format="TIFF")
render("tiff8_rgb_none", file_format="TIFF", color_mode="RGB", tiff_codec="NONE")
render("tiff8_bw_lzw", file_format="TIFF", color_mode="BW", tiff_codec="LZW")
render("tiff8_rgba_packbits", file_format="TIFF", tiff_codec="PACKBITS")
render("tiff16_rgba", file_format="TIFF", color_depth="16")
render("tiff16_rgb_lzw", file_format="TIFF", color_mode="RGB", color_depth="16", tiff_codec="LZW")
render("tiff16_bw_packbits", file_format="TIFF", color_mode="BW", color_depth="16", tiff_codec="PACKBITS")
render("tiff16_rgba_none", file_format="TIFF", color_depth="16", tiff_codec="NONE")
render("tiff16_rgba_agx", file_format="TIFF", color_depth="16", view="AgX")
render("iris_rgba", file_format="IRIS")
render("iris_rgb", file_format="IRIS", color_mode="RGB")
render("iris_bw", file_format="IRIS", color_mode="BW")
render("iris_rgba_opaque", transparent=False, file_format="IRIS")
render("dpx8_rgba", file_format="DPX")
render("dpx8_rgb", file_format="DPX", color_mode="RGB")
render("dpx10_rgb", file_format="DPX", color_mode="RGB", color_depth="10")
render("dpx10_rgba", file_format="DPX", color_depth="10")
render("dpx10_bw", file_format="DPX", color_mode="BW", color_depth="10")
render("dpx12_rgb", file_format="DPX", color_mode="RGB", color_depth="12")
render("dpx16_rgba", file_format="DPX", color_depth="16")
render("dpx16_bw", file_format="DPX", color_mode="BW", color_depth="16")
render("dpx10_rgb_log", file_format="DPX", color_mode="RGB", color_depth="10", use_cineon_log=True)
render("cineon_rgb", file_format="CINEON", color_mode="RGB", color_depth="10")
render("cineon_bw", file_format="CINEON", color_mode="BW", color_depth="10")
render("cineon_rgb_log", file_format="CINEON", color_mode="RGB", color_depth="10", use_cineon_log=True)
render("exrml32_rgba", file_format="OPEN_EXR_MULTILAYER", color_depth="32")
render("exrml16_rgb", file_format="OPEN_EXR_MULTILAYER", color_mode="RGB", color_depth="16")
render("exrml32_bw_piz", file_format="OPEN_EXR_MULTILAYER", color_mode="BW", color_depth="32", exr_codec="PIZ")
render("exrml32_opaque", transparent=False, file_format="OPEN_EXR_MULTILAYER", color_depth="32")



def read_exr(name):
    inp = oiio.ImageInput.open(os.path.join(OUT, name))
    pixels = inp.read_image(oiio.FLOAT)
    inp.close()
    return pixels


assert np.array_equal(read_exr("film_transparent.exr"), read_exr("check_transparent.exr"))
os.remove(os.path.join(OUT, "check_transparent.exr"))
manifest = [e for e in manifest if e["name"] != "check_transparent"]

# Noise saved as PNG and JPEG from one byte buffer (Standard view: bytes pass unchanged).
random.seed(7)
NW, NH = 75, 43
noise = bpy.data.images.new("Noise", NW, NH, alpha=False)
pixels = []
for i in range(NW * NH):
    ramp = (i % NW) / NW
    pixels += [min(1.0, max(0.0, ramp + random.uniform(-0.3, 0.3))), random.random(),
               (i // NW) / NH, 1.0]
noise.pixels.foreach_set(pixels)
scene.view_settings.view_transform = "Standard"
s = scene.render.image_settings
s.media_type = "IMAGE"
s.file_format = "PNG"
s.color_mode = "RGB"
s.color_depth = "8"
noise.save_render(os.path.join(OUT, "noise.png"))
s.file_format = "JPEG"
s.color_mode = "RGB"
for q in (95, 30):
    s.quality = q
    noise.save_render(os.path.join(OUT, f"noise_q{q}.jpg"))

with open(os.path.join(OUT, "manifest.json"), "w", newline="\n") as f:
    json.dump(
        {
            "generator": "tests/golden/fixtures/render/output/film/generate.py",
            "blender": bpy.app.version_string,
            "width": W,
            "height": H,
            "outputs": manifest,
        },
        f,
        indent=1,
    )
    f.write("\n")
