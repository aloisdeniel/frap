# WP 7.2: frap decodes Blender's Draco. The inputs are Blender 5.2.4's exports of
# `export_basic.py` (`blender_default.glb`: defaults, a shared primitive, two primitives in a
# mesh, split bowtie vertices) and `export_attributes.py` (`blender_attrs.glb`: COLOR_n,
# TANGENT, a morph target next to a sequentially encoded primitive, LINES and POINTS left
# uncompressed; `blender_generic8.glb`: level 10, 8 generic and 4 colour bits;
# `blender_skin.glb`: JOINTS_0/WEIGHTS_0); both tools import the same files. Compared: the
# objects (skin, vertex groups, shape keys included) and the meshes whose normals are
# axis-aligned or survive the importer's rounding; the smooth spheres, the slanted 'Two' and
# the smooth 'Bow' are not (deviation D-132, Draco and fused multiply-adds: their decoded values
# are checked within 1e-6 against Blender's decoder in frap-io-gltf
# `import/decompress/draco.rs`).
# golden: startup=empty tier=T2 tol=1e-5 order=canonical compare=objects,mesh:Cube,mesh:Cube.001,mesh:Grid,mesh:Loose,mesh:Cube.002,mesh:Grid.001,mesh:Loose.001
import bpy
bpy.ops.import_scene.gltf(filepath='blender_default.glb')
bpy.ops.import_scene.gltf(filepath='blender_attrs.glb')
bpy.ops.import_scene.gltf(filepath='blender_generic8.glb')
bpy.ops.import_scene.gltf(filepath='blender_skin.glb')
