# WP 2.8: mesh.fill triangulates closed selected edge loops (scan-fill, then beautify): a
# circle without fill, an open box's hole (winding from the neighbours), the same without
# beauty; the new faces and edges are selected. No selected edge raises
# `RuntimeError: Error: No edges selected`.
# golden: startup=empty compare=objects,context,mesh:Circle,mesh:Box tier=T1
import bpy
bpy.ops.mesh.primitive_circle_add(vertices=8, radius=1, fill_type='NOTHING')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.fill()
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.fill()
bpy.data.meshes['Circle'].total_face_sel
bpy.ops.mesh.primitive_circle_add(vertices=7, radius=0.5, fill_type='NOTHING', location=(3, 0, 0.5))
bpy.ops.mesh.fill(use_beauty=False)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes.new('Box').from_pydata([(-1, -1, -1), (1, -1, -1), (1, 1, -1), (-1, 1, -1), (-1, -1, 1), (1, -1, 1), (1, 1, 1), (-1, 1, 1)], [], [(0, 3, 2, 1), (0, 1, 5, 4), (1, 2, 6, 5), (2, 3, 7, 6), (3, 0, 4, 7)])
bpy.data.objects.new('Box', bpy.data.meshes['Box'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Box'])
bpy.context.view_layer.objects.active = bpy.data.objects['Box']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[4, 5, 6, 7])
bpy.ops.mesh.fill()
