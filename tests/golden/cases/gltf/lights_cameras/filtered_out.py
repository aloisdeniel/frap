# WP 3.4: by default (export_cameras=False, export_lights=False) cameras and lights are
# filtered out and their kept children move up (here a cube under the camera under the light
# becomes a root, with its world transform); a camera with export_lights only stays filtered.
# golden: startup=factory tier=T2 compare=glb:out/default.glb,glb:out/lights_only.glb glb_structure
import bpy
bpy.data.objects['Camera'].parent = bpy.data.objects['Light']
bpy.data.objects['Cube'].parent = bpy.data.objects['Camera']
bpy.ops.export_scene.gltf(filepath='out/default.glb')
bpy.ops.export_scene.gltf(filepath='out/lights_only.glb', export_lights=True)
