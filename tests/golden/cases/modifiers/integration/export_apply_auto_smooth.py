# WP 4.7: shade_auto_smooth adds the "Smooth by Angle" geometry-nodes modifier; export_apply
# writes its result (sharp edges above the angle split the normals) while the plain export
# writes the data (every face smooth). Linked duplicates of the cylinder without the modifier
# keep sharing the data mesh in the applied file.
# golden: startup=factory tier=T2 compare=glb:out/applied.glb,glb:out/data.glb tol=1e-4 glb_structure
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=8, location=(3, 0, 0))
bpy.ops.object.shade_auto_smooth(angle=0.6)
bpy.data.objects.new('Twin', bpy.data.meshes['Cylinder'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Twin'])
bpy.data.objects['Twin'].location = (-3, 0, 0)
bpy.data.objects.new('Twin2', bpy.data.meshes['Cylinder'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Twin2'])
bpy.data.objects['Twin2'].location = (0, 3, 0)
bpy.ops.export_scene.gltf(filepath='out/applied.glb', export_apply=True)
bpy.ops.export_scene.gltf(filepath='out/data.glb')
