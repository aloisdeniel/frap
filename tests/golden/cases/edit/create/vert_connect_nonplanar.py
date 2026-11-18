# WP 2.8: mesh.vert_connect_nonplanar splits selected faces bent more than the angle limit
# along their flattest diagonal, recursively (a folded hexagon and a twisted quad); a flat
# face is left alone.
# golden: startup=empty compare=objects,context,mesh:Bent tier=T1
import bpy
bpy.data.meshes.new('Bent').from_pydata([(0, 0, 0), (1, 0, 0), (2, 0, 0.5), (2, 1, 0.5), (1, 1, 0), (0, 1, 0), (4, 0, 0), (5, 0, 0.3), (5, 1, 0), (4, 1, 0.3), (7, 0, 0), (8, 0, 0), (8, 1, 0), (7, 1, 0)], [], [(0, 1, 2, 3, 4, 5), (6, 7, 8, 9), (10, 11, 12, 13)])
bpy.data.objects.new('Bent', bpy.data.meshes['Bent'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Bent'])
bpy.context.view_layer.objects.active = bpy.data.objects['Bent']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.vert_connect_nonplanar()
bpy.data.meshes['Bent'].total_face_sel
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.vert_connect_nonplanar(angle_limit=0.6)
