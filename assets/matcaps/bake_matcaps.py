# SPDX-License-Identifier: GPL-3.0-or-later
"""Bake Blender's Workbench matcaps into `assets/matcaps/` (WP 2.3).

Run with Blender 5.2.4 LTS (its bundled OpenImageIO reads the multilayer EXRs):
    Blender -b --factory-startup --python assets/matcaps/bake_matcaps.py -- assets/matcaps

Every `bpy.context.preferences.studio_lights` entry of type MATCAP is a datafile
`datafiles/studiolights/matcap/<name>.exr` (512 x 512, half float, lossy DWAA). Blender
(`BKE_studiolight.cc`, `studiolight_load_equirect_image`) uses the first `diffuse` pass of a
multilayer file as the diffuse matcap and the first `specular` pass as the specular one; a
single-layer file is the diffuse matcap and has no specular pass. The Workbench uploads them
as `SFLOAT_16_16_16_16` texture layers (`workbench_resources.cc`, `get_matcap_tx`) and only
reads `.rgb`.

The DWAA files decode to noisy half floats that compress badly losslessly (17 MB for all
passes at 512 x 512), so each pass is reduced to 256 x 256 with a 2 x 2 box average (in
linear float) and written as a single-layer RGB half-float ZIP EXR (the format
`frap_imbuf::decode` reads) named `<name>.diffuse.exr` / `<name>.specular.exr`. Blender renders
with these reduced matcaps (loaded as user matcaps) differ from renders with the shipped ones
by at most 2/255 per channel at 640 x 360 (measured on basic_bright, check_rim_light and
metal_carpaint). `MANIFEST.txt` lists every matcap in Blender's `preferences.studio_lights`
order with its passes. Provenance and licence: `README.md`.
"""
import os
import sys

import bpy
import numpy as np
import OpenImageIO as oiio

SIZE = 256

out_dir = sys.argv[sys.argv.index("--") + 1]
os.makedirs(out_dir, exist_ok=True)

manifest = []
for sl in bpy.context.preferences.studio_lights:
    if sl.type != "MATCAP":
        continue
    inp = oiio.ImageInput.open(sl.path)
    if inp is None:
        raise RuntimeError(f"cannot open {sl.path}: {oiio.geterror()}")
    spec = inp.spec()
    names = list(spec.channelnames)
    pixels = inp.read_image(0, 0, 0, spec.nchannels, oiio.FLOAT)
    inp.close()
    h, w = pixels.shape[0], pixels.shape[1]
    passes = {}
    for pass_name in ("diffuse", "specular"):
        chans = [f"{pass_name}.{c}" for c in "RGB"]
        if all(c in names for c in chans):
            passes[pass_name] = [names.index(c) for c in chans]
    if "diffuse" not in passes and all(c in names for c in "RGB"):
        # Single-layer file: the whole image is the diffuse matcap.
        passes = {"diffuse": [names.index(c) for c in "RGB"]}
    entry = [sl.name]
    for pass_name, idx in passes.items():
        rgb = np.ascontiguousarray(pixels[:, :, idx], dtype=np.float32)
        f = w // SIZE
        rgb = rgb.reshape(SIZE, f, SIZE, f, 3).mean(axis=(1, 3), dtype=np.float32)
        buf = oiio.ImageBuf(oiio.ImageSpec(SIZE, SIZE, 3, oiio.HALF))
        buf.set_pixels(oiio.ROI(0, SIZE, 0, SIZE, 0, 1, 0, 3), np.ascontiguousarray(rgb))
        buf.specmod().attribute("compression", "zip")
        dst = os.path.join(out_dir, f"{sl.name[:-4]}.{pass_name}.exr")
        if not buf.write(dst, oiio.HALF):
            raise RuntimeError(f"cannot write {dst}: {buf.geterror()}")
        entry.append(pass_name)
        print("wrote", dst)
    manifest.append(" ".join(entry))

with open(os.path.join(out_dir, "MANIFEST.txt"), "w") as f:
    f.write("\n".join(manifest) + "\n")
