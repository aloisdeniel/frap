# WP 7.5: EXT_mesh_gpu_instancing import (`manage_gpu_instancing`): each instance becomes a
# mesh object (named after the mesh, `Cube.001`, …, sharing one Blender mesh) parented to the
# holder, which is an empty; the instances are not selected. A Blender export with
# `export_gpu_instances` is imported into an empty file and exported again without the prop
# (every instance a node).
# golden: startup=factory tier=T2 compare=objects,context,glb:out/again.glb glb_structure
import bpy
bpy.ops.object.empty_add(location=(0, 0, 1))
bpy.data.objects['Cube'].parent = bpy.data.objects['Empty']
bpy.data.objects['Cube'].location = (1, 0, 0)
bpy.data.objects.new('Cube2', bpy.data.meshes['Cube'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Cube2'])
bpy.data.objects['Cube2'].parent = bpy.data.objects['Empty']
bpy.data.objects['Cube2'].location = (-1, 2, 0)
bpy.data.objects['Cube2'].rotation_euler = (0.3, 0.0, 1.2)
bpy.data.objects['Cube2'].scale = (0.5, 0.5, 2.0)
bpy.data.objects.new('Cube3', bpy.data.meshes['Cube'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Cube3'])
bpy.data.objects['Cube3'].parent = bpy.data.objects['Empty']
bpy.data.objects['Cube3'].location = (0, -3, 0.5)
bpy.ops.export_scene.gltf(filepath='out/inst.glb', export_gpu_instances=True)
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/inst.glb')
bpy.data.objects['Empty'].location = (bpy.data.objects['Empty'].empty_display_size, 0.0, 0.0)
bpy.data.objects['Empty'].name = bpy.data.objects['Cube.002'].data.name + '|' + bpy.data.objects['Cube'].data.name
bpy.ops.export_scene.gltf(filepath='out/again.glb')
