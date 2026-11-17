# WP 1.23: import of a GLTF_SEPARATE export (external .bin) through `files` + `directory`, and
# the operator's 'Please select a file' error for a missing path.
# Each tool imports its own export, whose vertex order may legitimately differ
# (glTF outputs compare canonically), so meshes compare canonically.
# golden: order=canonical startup=factory tier=T1 compare=objects,mesh:Cube tol=1e-5
import bpy
bpy.ops.export_scene.gltf(filepath='out/sep/cube.gltf', export_format='GLTF_SEPARATE')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/missing.glb')
bpy.ops.import_scene.gltf(filepath='out/sep/cube.gltf')
