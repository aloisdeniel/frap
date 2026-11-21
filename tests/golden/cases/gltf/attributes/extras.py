# WP 3.7: export_extras. Off by default, nothing is written even with custom properties; on, a
# scene without custom properties writes no extras either. Custom properties authored in both
# tools and exported with export_extras: `extras_authored.py` (ID properties, M3+M4
# stabilization round 2); collection exports: `export::extras::tests` in frap-io-gltf.
# golden: startup=factory tier=T2 compare=glb:out/off.glb,glb:out/bare.glb glb_structure
import bpy
bpy.data.objects['Cube']['int'] = 3
bpy.data.meshes['Cube']['mesh_prop'] = 'm'
bpy.data.materials['Material']['mat_prop'] = 7
bpy.data.scenes['Scene']['scene_prop'] = 'scene'
bpy.ops.export_scene.gltf(filepath='out/off.glb')
bpy.ops.wm.read_homefile(use_empty=False)
bpy.ops.export_scene.gltf(filepath='out/bare.glb', export_extras=True)
