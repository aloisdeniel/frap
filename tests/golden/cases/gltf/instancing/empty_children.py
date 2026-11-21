# WP 7.5: `export_gpu_instances=True` with the add-on's rule (`manage_gpu_instancing`): the
# childless children of an Empty that share a mesh become one EXT_mesh_gpu_instancing holder.
# One group under an Empty: the Empty itself holds the instances (it gets the mesh and the
# TRANSLATION / ROTATION / SCALE accessors of the children's local TRS), the instance nodes
# are removed. A child with its own child, and a child with another mesh, stay nodes. Without
# the prop the same scene keeps every node.
# golden: startup=factory tier=T2 compare=glb:out/inst.glb,glb:out/plain.glb glb_structure
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
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=4, location=(0, 0, 4))
bpy.data.objects['Sphere'].parent = bpy.data.objects['Empty']
bpy.data.objects.new('Cube4', bpy.data.meshes['Cube'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Cube4'])
bpy.data.objects['Cube4'].parent = bpy.data.objects['Empty']
bpy.ops.object.empty_add(location=(0, 1, 0))
bpy.data.objects['Empty.001'].parent = bpy.data.objects['Cube4']
bpy.ops.export_scene.gltf(filepath='out/inst.glb', export_gpu_instances=True)
bpy.ops.export_scene.gltf(filepath='out/plain.glb')
