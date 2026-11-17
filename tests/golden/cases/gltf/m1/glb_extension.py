# WP 1.23: `export_format='GLB'` with a path without extension and with a `.gltf` one: the
# add-on forces `.glb`.
# golden: startup=factory tier=T2 compare=glb:out/noext.glb,glb:out/other.glb
import bpy
bpy.ops.export_scene.gltf(filepath='out/noext', export_format='GLB')
bpy.ops.export_scene.gltf(filepath='out/other.gltf')
