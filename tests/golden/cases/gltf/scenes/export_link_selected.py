# WP 3.10: an object selected in the first scene and linked into a second one is selected
# there too (its new base takes the object's stored selection), so a `use_selection` export of
# the second scene keeps it.
# golden: startup=factory tier=T2 compare=glb:out/selected.glb,glb:out/all.glb
import bpy
bpy.data.scenes.new('S2')
bpy.data.scenes['S2'].collection.objects.link(bpy.data.objects['Cube'])
bpy.data.scenes['S2'].collection.objects.link(bpy.data.objects['Light'])
bpy.context.window.scene = bpy.data.scenes['S2']
bpy.ops.export_scene.gltf(filepath='out/selected.glb', use_selection=True)
bpy.ops.export_scene.gltf(filepath='out/all.glb')
