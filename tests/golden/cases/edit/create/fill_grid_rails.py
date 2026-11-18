# WP 2.8: mesh.fill_grid between two open chains joined by wire rails (grid_fill finds the
# rails), with chains of different lengths (the shorter one is padded and the padding edges
# collapsed afterwards), and next to faces (winding from the attached boundary faces).
# golden: startup=empty compare=objects,context,mesh:Rails,mesh:Uneven tier=T1
import bpy
bpy.data.meshes.new('Rails').from_pydata([(0, 0, 0), (1, 0, 0), (2, 0, 0), (3, 0, 0), (0, 2, 0.3), (1, 2, 0.3), (2, 2, 0.3), (3, 2, 0.3), (0, 1, 0.1), (3, 1, 0.1), (0, -1, 0), (1, -1, 0)], [(0, 1), (1, 2), (2, 3), (4, 5), (5, 6), (6, 7), (0, 8), (8, 4), (3, 9), (9, 7)], [(10, 11, 1, 0)])
bpy.data.objects.new('Rails', bpy.data.meshes['Rails'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Rails'])
bpy.context.view_layer.objects.active = bpy.data.objects['Rails']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 1, 2, 3, 4, 5])
bpy.ops.mesh.fill_grid()
bpy.data.meshes['Rails'].total_face_sel
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes.new('Uneven').from_pydata([(0, 0, 0), (1, 0, 0), (2, 0, 0), (3, 0, 0), (0, 2, 0), (1.5, 2, 0), (3, 2, 0), (0, 1, 0), (3, 1, 0)], [(0, 1), (1, 2), (2, 3), (4, 5), (5, 6), (0, 7), (7, 4), (3, 8), (8, 6)], [])
bpy.data.objects.new('Uneven', bpy.data.meshes['Uneven'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Uneven'])
bpy.context.view_layer.objects.active = bpy.data.objects['Uneven']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 1, 2, 3, 4])
bpy.ops.mesh.fill_grid()
