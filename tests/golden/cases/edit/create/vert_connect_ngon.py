# WP 2.8: mesh.vert_connect with several vertices of one concave n-gon (connect_verts with
# check_degenerate: splits leaving the face or crossing are dropped), then vert_connect_path
# from an edge selection history (turned into a vertex path first).
# golden: startup=empty compare=objects,context,mesh:Star,mesh:Grid tier=T1
import bpy
bpy.data.meshes.new('Star').from_pydata([(0, 0, 0), (2, 0, 0), (2, 2, 0), (1.2, 0.8, 0), (0, 2, 0), (-0.5, 1, 0)], [], [(0, 1, 2, 3, 4, 5)])
bpy.data.objects.new('Star', bpy.data.meshes['Star'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Star'])
bpy.context.view_layer.objects.active = bpy.data.objects['Star']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 2, 4])
bpy.ops.mesh.vert_connect()
bpy.data.meshes['Star'].total_face_sel
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.select_all(action='DESELECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=5, y_subdivisions=5, size=2, location=(4, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[1, 26, 13])
bpy.ops.mesh.vert_connect_path()
