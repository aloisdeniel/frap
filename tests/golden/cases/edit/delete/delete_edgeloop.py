# WP 2.7: mesh.delete_edgeloop dissolves the selected edge loop (dissolve_edges with
# use_verts=True), merges the faces on both sides and selects them; with use_face_split the
# loop's end vertices on the boundary keep their corners.
# golden: startup=factory compare=objects,context,mesh:Grid,mesh:Grid.001 tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[1, 6, 11, 16, 21])
bpy.ops.mesh.delete_edgeloop()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4, location=(3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[6, 7, 8])
bpy.ops.mesh.delete_edgeloop(use_face_split=False)
