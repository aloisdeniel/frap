# SPDX-License-Identifier: GPL-3.0-or-later
"""Reference Workbench renders from Blender for `crates/frap-render/tests/workbench_ref.rs`.

Run (Blender 5.2.4 LTS):
    Blender -b --factory-startup --python crates/frap-render/tests/data/make_workbench_ref.py -- \
        crates/frap-render/tests/data/workbench_ref.txt

It builds a small scene (smooth UV sphere, flat cube with a negative scale, a plane without a
material), renders it with `BLENDER_WORKBENCH` at `render_aa='OFF'` (one sample at pixel
centres, so frap's single-sample render can be compared per pixel) for several
`display.shading` settings, saves each render as a 32-bit OpenEXR (scene-linear), loads it back
and writes a plain text fixture:

    camera view <16 floats, column-major> / camera proj <16 floats, OpenGL depth, column-major>
    object <name> <random_hash> <16 floats matrix_world> <r g b a object colour>
    material <slot> <r g b a diffuse_color> <metallic> <roughness>
    tri <9 position floats> <9 normal floats>   (object space, corner-split)
    case <name> <light> <color_type> <film_transparent> <single r g b>
    px <w*h*4 floats, top row first>

`random_hash` is `BLI_ghashutil_strhash_p_murmur(id.name)`: MurmurHash2 (seed 0) of the `OB`-prefixed
name *including its terminating NUL* (`strlen + 1` bytes), computed
here in Python with the same MurmurHash2 so the fixture pins the convention frap uses.
"""
import struct
import sys

import bpy

W, H = 40, 30
out_path = sys.argv[sys.argv.index("--") + 1]


def mm2(data: bytes) -> int:
    """BLI_hash_mm2(data, len, 0) (MurmurHash2, Blender `hash_mm2a.cc` mm2 variant)."""
    m = 0x5BD1E995
    mask = 0xFFFFFFFF
    n = len(data)
    h = (0 ^ n) & mask
    i = 0
    while n >= 4:
        k = struct.unpack_from("<I", data, i)[0]
        k = (k * m) & mask
        k ^= k >> 24
        k = (k * m) & mask
        h = (h * m) & mask
        h ^= k
        i += 4
        n -= 4
    if n == 3:
        h ^= data[i + 2] << 16
    if n >= 2:
        h ^= data[i + 1] << 8
    if n >= 1:
        h ^= data[i]
        h = (h * m) & mask
    h ^= h >> 13
    h = (h * m) & mask
    h ^= h >> 15
    return h


for ob in list(bpy.data.objects):
    if ob.type != "CAMERA":
        bpy.data.objects.remove(ob)

scene = bpy.context.scene
cam = scene.camera
cam.location = (6.0, -7.0, 4.5)
cam.rotation_euler = (1.13, 0.0, 0.70)
scene.render.engine = "BLENDER_WORKBENCH"
scene.render.resolution_x = W
scene.render.resolution_y = H
scene.render.resolution_percentage = 100
scene.display.render_aa = "OFF"
scene.render.image_settings.file_format = "OPEN_EXR"
scene.render.image_settings.color_depth = "32"
scene.render.image_settings.exr_codec = "NONE"

bpy.ops.mesh.primitive_uv_sphere_add(segments=16, ring_count=8, radius=1.0, location=(0.0, 0.0, 1.0))
sphere = bpy.context.object
sphere.name = "Sphere"
bpy.ops.object.shade_smooth()
sphere.color = (0.9, 0.3, 0.2, 1.0)
ma = bpy.data.materials.new("Red")
ma.diffuse_color = (0.8, 0.15, 0.1, 1.0)
ma.metallic = 0.0
ma.roughness = 0.3
sphere.data.materials.append(ma)

bpy.ops.mesh.primitive_cube_add(size=1.0, location=(2.0, 1.0, 0.75))
cube = bpy.context.object
cube.name = "Cube"
cube.scale = (-1.0, 1.0, 1.5)
cube.rotation_euler = (0.0, 0.0, 0.4)
cube.color = (0.2, 0.7, 0.3, 1.0)
mb = bpy.data.materials.new("Blue")
mb.diffuse_color = (0.1, 0.4, 0.9, 1.0)
mb.metallic = 0.7
mb.roughness = 0.6
cube.data.materials.append(mb)

bpy.ops.mesh.primitive_plane_add(size=8.0, location=(0.0, 0.0, 0.0))
plane = bpy.context.object
plane.name = "Plane"

bpy.context.view_layer.update()
dg = bpy.context.evaluated_depsgraph_get()


def fl(v):
    return " ".join(f"{x:.7g}" for x in v)


def mat_cols(m):
    return [m[r][c] for c in range(4) for r in range(4)]


lines = []
lines.append("size %d %d" % (W, H))
lines.append("camera view " + fl(mat_cols(cam.matrix_world.inverted())))
lines.append(
    "camera proj " + fl(mat_cols(cam.calc_matrix_camera(dg, x=W, y=H, scale_x=1.0, scale_y=1.0)))
)
for ob in sorted([sphere, cube, plane], key=lambda o: o.name):
    me = ob.data
    me.calc_loop_triangles()
    lines.append(
        "object %s %d %s %s"
        % (ob.name, mm2(("OB" + ob.name).encode() + b"\0"), fl(mat_cols(ob.matrix_world)), fl(ob.color))
    )
    for slot, m in enumerate(me.materials):
        lines.append(
            "material %d %s %.7g %.7g" % (slot, fl(m.diffuse_color), m.metallic, m.roughness)
        )
    cn = me.corner_normals
    for t in me.loop_triangles:
        p = []
        n = []
        for li in t.loops:
            p += list(me.vertices[me.loops[li].vertex_index].co)
            n += list(cn[li].vector)
        lines.append("tri " + fl(p) + " " + fl(n))

CASES = [
    ("studio_material", "STUDIO", "MATERIAL", False, (0.8, 0.8, 0.8)),
    ("flat_material", "FLAT", "MATERIAL", False, (0.8, 0.8, 0.8)),
    ("studio_single", "STUDIO", "SINGLE", False, (0.3, 0.6, 0.9)),
    ("studio_object", "STUDIO", "OBJECT", False, (0.8, 0.8, 0.8)),
    ("flat_random", "FLAT", "RANDOM", False, (0.8, 0.8, 0.8)),
    ("flat_single_transparent", "FLAT", "SINGLE", True, (0.25, 0.5, 0.75)),
]
tmp = bpy.app.tempdir + "frap_wb_ref.exr"
for name, light, color_type, transparent, single in CASES:
    sh = scene.display.shading
    sh.light = light
    sh.color_type = color_type
    sh.single_color = single
    scene.render.film_transparent = transparent
    scene.render.filepath = tmp
    bpy.ops.render.render(write_still=True)
    img = bpy.data.images.load(tmp, check_existing=False)
    px = list(img.pixels)
    w, h = img.size
    assert (w, h) == (W, H), (w, h)
    rows = []
    for y in range(h - 1, -1, -1):  # Blender pixels are bottom row first
        rows += px[y * w * 4 : (y + 1) * w * 4]
    bpy.data.images.remove(img)
    lines.append(
        "case %s %s %s %d %s" % (name, light, color_type, int(transparent), fl(single))
    )
    lines.append("px " + " ".join(f"{x:.5g}" for x in rows))

with open(out_path, "w") as f:
    f.write("# Generated by make_workbench_ref.py on Blender %s\n" % bpy.app.version_string)
    f.write("\n".join(lines) + "\n")
print("wrote", out_path)
