# WP 2.8: mesh.fill_grid on one closed loop (split into two rails at the sharpest corner,
# span from the corners or a quarter of the loop), with an explicit span and offset, and on a
# selection that is not one or two loops (INFO report, FINISHED, nothing changes).
# golden: startup=empty compare=objects,context,mesh:Circle,mesh:Ring tier=T2
import bpy
bpy.ops.mesh.primitive_circle_add(vertices=8, radius=1, fill_type='NOTHING')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.fill_grid()
bpy.data.meshes['Circle'].total_face_sel
bpy.ops.mesh.primitive_circle_add(vertices=12, radius=1, fill_type='NOTHING', location=(3, 0, 0))
bpy.ops.mesh.fill_grid(span=2, offset=1)
bpy.ops.mesh.primitive_circle_add(vertices=16, radius=1, fill_type='NOTHING', location=(-3, 0, 0))
bpy.ops.mesh.fill_grid(use_interp_simple=True)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.select_all(action='DESELECT')
bpy.data.meshes.new('Ring').from_pydata([(0, 0, 0), (1, 0, 0), (2, 0, 0), (3, 0, 0), (3, 1, 0), (3, 2, 0), (2, 2, 0), (1, 2, 0), (0, 2, 0), (0, 1, 0), (5, 0, 0), (6, 0, 0)], [(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 6), (6, 7), (7, 8), (8, 9), (9, 0), (10, 11)], [])
bpy.data.objects.new('Ring', bpy.data.meshes['Ring'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Ring'])
bpy.context.view_layer.objects.active = bpy.data.objects['Ring']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.fill_grid()
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 1, 2, 3, 4, 5, 6, 7, 8, 9])
bpy.ops.mesh.fill_grid()
