# WP 7.5: `export_gpu_instances` holder nodes. Two groups of instances under one Empty (a cube
# mesh and a cone mesh, two children each) give two new child nodes `Empty.0` and `Empty.1`;
# a mesh object with two childless children sharing a mesh gets one new child `Parent.0`
# (`also_mesh`: the parent keeps its own mesh). A lone child of a mesh stays a node. Deeper
# levels are handled too (the recursion continues into the remaining children).
# golden: startup=factory tier=T2 compare=glb:out/holders.glb glb_structure
import bpy
bpy.ops.object.empty_add(location=(0, 0, 0))
bpy.data.objects['Cube'].parent = bpy.data.objects['Empty']
bpy.data.objects.new('CubeB', bpy.data.meshes['Cube'])
bpy.context.scene.collection.objects.link(bpy.data.objects['CubeB'])
bpy.data.objects['CubeB'].parent = bpy.data.objects['Empty']
bpy.data.objects['CubeB'].location = (3, 0, 0)
bpy.ops.mesh.primitive_cone_add(vertices=6, location=(0, 3, 0))
bpy.data.objects['Cone'].parent = bpy.data.objects['Empty']
bpy.data.objects.new('ConeB', bpy.data.meshes['Cone'])
bpy.context.scene.collection.objects.link(bpy.data.objects['ConeB'])
bpy.data.objects['ConeB'].parent = bpy.data.objects['Empty']
bpy.data.objects['ConeB'].location = (3, 3, 1)
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=1, location=(0, 0, 6))
bpy.context.object.name = 'Parent'
bpy.ops.mesh.primitive_plane_add(size=0.5, location=(1, 0, 6))
bpy.context.object.name = 'P1'
bpy.data.objects['P1'].parent = bpy.data.objects['Parent']
bpy.data.objects.new('P2', bpy.data.meshes['Plane'])
bpy.context.scene.collection.objects.link(bpy.data.objects['P2'])
bpy.data.objects['P2'].parent = bpy.data.objects['Parent']
bpy.data.objects['P2'].location = (-1, 0, 0)
bpy.data.objects.new('P3', bpy.data.meshes['Plane'])
bpy.context.scene.collection.objects.link(bpy.data.objects['P3'])
bpy.data.objects['P3'].parent = bpy.data.objects['Parent']
bpy.data.objects['P3'].location = (0, 1, 0.5)
bpy.data.objects.new('Lone', bpy.data.meshes['Cone'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Lone'])
bpy.data.objects['Lone'].parent = bpy.data.objects['P2']
bpy.ops.export_scene.gltf(filepath='out/holders.glb', export_gpu_instances=True)
