# WP 3.10: every Blender scene becomes a glTF scene (named after it) with the window's scene
# as the default `scene`; `use_active_scene=True` exports the window's scene only. Each
# scene's nodes come from that scene's objects.
# golden: startup=factory tier=T2 compare=glb:out/all.glb,glb:out/active.glb,glb:out/second.glb,glb:out/second_active.glb glb_structure
import bpy
bpy.data.scenes.new('S2')
bpy.context.window.scene = bpy.data.scenes['S2']
bpy.ops.mesh.primitive_cylinder_add(vertices=8, location=(0, 0, 2))
bpy.ops.object.empty_add(location=(1, 0, 0))
bpy.data.objects['Cylinder'].parent = bpy.data.objects['Empty']
bpy.context.window.scene = bpy.data.scenes['Scene']
bpy.ops.export_scene.gltf(filepath='out/all.glb')
bpy.ops.export_scene.gltf(filepath='out/active.glb', use_active_scene=True)
bpy.context.window.scene = bpy.data.scenes['S2']
bpy.ops.export_scene.gltf(filepath='out/second.glb')
bpy.ops.export_scene.gltf(filepath='out/second_active.glb', use_active_scene=True)
bpy.context.window.scene = bpy.data.scenes['Scene']
