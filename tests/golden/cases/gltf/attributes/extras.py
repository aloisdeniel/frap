# WP 3.7: export_extras. Off by default, nothing is written even with custom properties; on, a
# scene without custom properties writes no extras either. The custom-property statements run
# in Blender only (`ID[key] = value` arrives in frap with ID properties, M8), so frap's
# extras values (objects, meshes, materials, scenes, collection exports) are checked against
# Blender's for the same properties by `export::extras::tests` in frap-io-gltf.
# golden: startup=factory tier=T2 compare=glb:out/off.glb,glb:out/bare.glb
import bpy
#@blender bpy.data.objects['Cube']['int'] = 3
#@blender bpy.data.meshes['Cube']['mesh_prop'] = 'm'
#@blender bpy.data.materials['Material']['mat_prop'] = 7
#@blender bpy.data.scenes['Scene']['scene_prop'] = 'scene'
bpy.ops.export_scene.gltf(filepath='out/off.glb')
bpy.ops.wm.read_homefile(use_empty=False)
bpy.ops.export_scene.gltf(filepath='out/bare.glb', export_extras=True)
