# M7 W1 integration (WPs 7.1 + 7.5): `export_gpu_instances` with meshopt compression. The
# instance TRANSLATION / SCALE views are EXPONENTIAL-filtered and ROTATION is QUATERNION-
# filtered (its accessor becomes normalized SHORT, its fallback view keeps the FLOAT length,
# as `manage_gpu_instancing_nodes`), with EXT and with KHR_meshopt_compression. Each tool
# imports its own files back (the canonical glTF form does not decode meshopt views) and
# exports them again uncompressed; the compressed structure (filters, strides, fallback
# lengths) was checked against Blender 5.2.4 by hand at the M7 W1 integration.
# golden: startup=factory tier=T2 tol=1e-5 compare=objects,glb:out/ext_again.glb,glb:out/khr_again.glb glb_structure
import bpy
bpy.ops.object.empty_add(location=(0, 0, 0))
bpy.data.objects['Cube'].parent = bpy.data.objects['Empty']
bpy.data.objects.new('Cube2', bpy.data.meshes['Cube'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Cube2'])
bpy.data.objects['Cube2'].parent = bpy.data.objects['Empty']
bpy.data.objects['Cube2'].location = (3, 0, 0)
bpy.data.objects['Cube2'].rotation_euler = (0.3, 0, 0.2)
bpy.data.objects.new('Cube3', bpy.data.meshes['Cube'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Cube3'])
bpy.data.objects['Cube3'].parent = bpy.data.objects['Empty']
bpy.data.objects['Cube3'].location = (0, 4, 1)
bpy.data.objects['Cube3'].scale = (0.5, 2, 1)
bpy.ops.export_scene.gltf(filepath='out/ext.glb', export_gpu_instances=True, export_meshopt_compression_enable=True)
bpy.ops.export_scene.gltf(filepath='out/khr.glb', export_gpu_instances=True, export_meshopt_compression_enable=True, export_meshopt_extension='KHR_meshopt_compression')
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/ext.glb')
bpy.ops.export_scene.gltf(filepath='out/ext_again.glb')
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/khr.glb')
bpy.ops.export_scene.gltf(filepath='out/khr_again.glb')
