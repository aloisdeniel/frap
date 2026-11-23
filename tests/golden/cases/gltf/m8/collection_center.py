# WP 8.12: `at_collection_center` with `collection`: the root nodes of the exported collection
# (objects, armatures, lights, cameras after filtering) are moved by minus the mean of their
# world translations (`VExportTree.calculate_collection_center`, then `__gather_trans_rot_scale`);
# children keep their local transforms. Without `collection` the option does nothing, and
# `export_original_specular` (kept by the add-on for compatibility, never read) changes nothing.
# golden: startup=factory tier=T2 compare=glb:out/center.glb,glb:out/nocoll.glb,glb:out/spec.glb glb_structure
import bpy
bpy.data.objects['Cube'].location = (1.0, 2.0, 3.0)
bpy.data.objects['Cube'].rotation_euler = (0.3, 0.0, 0.5)
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=4, location=(-3.0, 1.0, 0.5))
bpy.ops.mesh.primitive_plane_add(location=(0.5, 0.5, 4.0))
bpy.data.objects['Plane'].parent = bpy.data.objects['Sphere']
bpy.data.collections.new('Other')
bpy.context.scene.collection.children.link(bpy.data.collections['Other'])
bpy.ops.mesh.primitive_cone_add(vertices=6, location=(9.0, 9.0, 9.0))
bpy.data.collections['Collection'].objects.unlink(bpy.data.objects['Cone'])
bpy.data.collections['Other'].objects.link(bpy.data.objects['Cone'])
bpy.ops.export_scene.gltf(filepath='out/center.glb', collection='Collection', at_collection_center=True, export_cameras=True, export_lights=True)
bpy.ops.export_scene.gltf(filepath='out/nocoll.glb', at_collection_center=True)
bpy.ops.export_scene.gltf(filepath='out/spec.glb', export_original_specular=True)
