# WP 2.8 (needs 2.10 triangulate): vert_connect_concave on a concave n-gon.
# golden: startup=empty compare=objects,context,mesh:L tier=T2
import bpy
bpy.data.meshes.new('L').from_pydata([(0, 0, 0), (2, 0, 0), (2, 1, 0), (1, 1, 0), (1, 2, 0), (0, 2, 0)], [], [(0, 1, 2, 3, 4, 5)])
bpy.data.objects.new('L', bpy.data.meshes['L'])
bpy.context.scene.collection.objects.link(bpy.data.objects['L'])
bpy.context.view_layer.objects.active = bpy.data.objects['L']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.vert_connect_concave()
bpy.data.meshes['L'].total_face_sel
