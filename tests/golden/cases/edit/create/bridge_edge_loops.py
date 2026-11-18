# WP 2.8: mesh.bridge_edge_loops between two parallel square wire loops (4 side quads,
# selected), with a twist offset on closed loops, three loops bridged in order with
# type='CLOSED', and the INFO error `Select at least two edge loops` (FINISHED, unchanged).
# golden: startup=empty compare=objects,context,mesh:Loops,mesh:Three tier=T1
import bpy
bpy.data.meshes.new('Loops').from_pydata([(-1, -1, 0), (1, -1, 0), (1, 1, 0), (-1, 1, 0), (-1, -1, 2), (1, -1, 2), (1, 1, 2), (-1, 1, 2), (4, 0, 0), (5, 0, 0), (5, 1, 0), (4, 1, 0), (4, 0, 1), (5, 0, 1), (5, 1, 1), (4, 1, 1)], [(0, 1), (1, 2), (2, 3), (3, 0), (4, 5), (5, 6), (6, 7), (7, 4), (8, 9), (9, 10), (10, 11), (11, 8), (12, 13), (13, 14), (14, 15), (15, 12)], [])
bpy.data.objects.new('Loops', bpy.data.meshes['Loops'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Loops'])
bpy.context.view_layer.objects.active = bpy.data.objects['Loops']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 1, 2, 3])
bpy.ops.mesh.bridge_edge_loops()
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 1, 2, 3, 4, 5, 6, 7])
bpy.ops.mesh.bridge_edge_loops()
bpy.data.meshes['Loops'].total_face_sel
bpy.ops.frap.select_elements(type='EDGE', indices=[8, 9, 10, 11, 12, 13, 14, 15])
bpy.ops.mesh.bridge_edge_loops(twist_offset=1)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.select_all(action='DESELECT')
bpy.data.meshes.new('Three').from_pydata([(0, 0, 0), (1, 0, 0), (0.5, 1, 0), (0, 0, 1), (1, 0, 1), (0.5, 1, 1), (0, 0, 2.5), (1, 0, 2.5), (0.5, 1, 2.5)], [(0, 1), (1, 2), (2, 0), (3, 4), (4, 5), (5, 3), (6, 7), (7, 8), (8, 6)], [])
bpy.data.objects.new('Three', bpy.data.meshes['Three'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Three'])
bpy.context.view_layer.objects.active = bpy.data.objects['Three']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bridge_edge_loops(type='CLOSED')
