# WP 1.23: argument errors of export_scene.gltf: GLTF_EMBEDDED needs the add-on preference
# allow_embedded_format (off in factory settings), unknown items and keywords, wrong types.
# golden: startup=factory tier=T0
import bpy
bpy.ops.export_scene.gltf(filepath='out/e.gltf', export_format='GLTF_EMBEDDED')
bpy.ops.export_scene.gltf(filepath='out/e.glb', export_format='FOO')
bpy.ops.export_scene.gltf(filepath='out/e.glb', export_draco=True)
bpy.ops.export_scene.gltf(filepath='out/e.glb', use_selection='yes')
