# WP 7.5: `export_gpu_instances` with animations. The channels of an animated instance are
# dropped with its node; the add-on does not renumber the channels that survive, so the
# animated sphere after the removed nodes keeps its old `target.node` (frap keeps the quirk
# under the default `frap_compat='BLENDER'`, deviations.md).
# golden: startup=factory tier=T2 compare=glb:out/anim.glb glb_structure
import bpy
bpy.ops.object.empty_add(location=(0, 0, 0))
bpy.data.objects['Cube'].parent = bpy.data.objects['Empty']
bpy.data.objects.new('Cube2', bpy.data.meshes['Cube'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Cube2'])
bpy.data.objects['Cube2'].parent = bpy.data.objects['Empty']
bpy.data.objects['Cube2'].location = (3, 0, 0)
bpy.data.objects['Cube2'].keyframe_insert('location', frame=1)
bpy.data.objects['Cube2'].location = (3, 0, 2)
bpy.data.objects['Cube2'].keyframe_insert('location', frame=10)
bpy.ops.mesh.primitive_uv_sphere_add(segments=8, ring_count=4, location=(0, 5, 0))
bpy.data.objects['Sphere'].keyframe_insert('location', frame=1)
bpy.data.objects['Sphere'].location = (0, 5, 3)
bpy.data.objects['Sphere'].keyframe_insert('location', frame=10)
bpy.ops.export_scene.gltf(filepath='out/anim.glb', export_gpu_instances=True)
