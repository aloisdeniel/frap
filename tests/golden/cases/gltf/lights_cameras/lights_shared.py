# WP 3.4: two objects sharing one light datablock share one KHR_lights_punctual entry; a
# copy of the data with another name is a separate entry; Z-up export (no correction
# rotation on cameras and lights).
# golden: startup=factory tier=T2 compare=glb:out/shared.glb,glb:out/zup.glb glb_structure
import bpy
bpy.data.objects.new('Lamp2', bpy.data.lights['Light'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Lamp2'])
bpy.data.objects['Lamp2'].location = (-3, 0, 2)
bpy.data.objects.new('Lamp3', bpy.data.lights['Light'].copy())
bpy.context.scene.collection.objects.link(bpy.data.objects['Lamp3'])
bpy.ops.export_scene.gltf(filepath='out/shared.glb', export_lights=True)
bpy.ops.export_scene.gltf(filepath='out/zup.glb', export_lights=True, export_cameras=True, export_yup=False)
