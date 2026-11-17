# SPDX-License-Identifier: GPL-3.0-or-later
"""Bake Blender's STUDIO studio lights into `assets/studio_lights/*.sl` (WP 1.7).

Run with Blender 5.2.4 LTS:
    Blender -b --factory-startup --python assets/studio_lights/bake_studio_lights.py -- assets/studio_lights

Every `bpy.context.preferences.studio_lights` entry of type STUDIO is written in Blender's own
`.sl` text format (`BKE_studiolight.cc` writer: `version 1`, `light_ambient.*`, `light[i].flag`,
`.smooth`, `.col.*`, `.spec.*`, `.vec.*`, values printed with `%f`). The built-in "Default" rig
(hard-coded in `BKE_studiolight.cc`, not a datafile) is written as `Default.sl`; the others
(`basic.sl`, `outdoor.sl`, `paint.sl`, `rim.sl`, `studio.sl`) come from
`datafiles/studiolights/studio/` and round-trip byte-identically through this writer. Provenance:
`assets/studio_lights/README.md`.
"""
import os
import sys

import bpy

out_dir = sys.argv[sys.argv.index("--") + 1]


def f(v):
    return "%f" % v


for sl in bpy.context.preferences.studio_lights:
    if sl.type != "STUDIO":
        continue
    name = sl.name if sl.name.endswith(".sl") else sl.name + ".sl"
    lines = ["version 1"]
    for c, v in zip("xyz", sl.light_ambient):
        lines.append("light_ambient.%s %s" % (c, f(v)))
    for i, l in enumerate(sl.solid_lights):
        lines.append("light[%d].flag %d" % (i, int(l.use)))
        lines.append("light[%d].smooth %s" % (i, f(l.smooth)))
        for key, vec in (("col", l.diffuse_color), ("spec", l.specular_color), ("vec", l.direction)):
            for c, v in zip("xyz", vec):
                lines.append("light[%d].%s.%s %s" % (i, key, c, f(v)))
    with open(os.path.join(out_dir, name), "w", newline="\n") as fh:
        fh.write("\n".join(lines) + "\n")
    print("baked", name, "specular_highlight_pass=%s" % sl.has_specular_highlight_pass)
