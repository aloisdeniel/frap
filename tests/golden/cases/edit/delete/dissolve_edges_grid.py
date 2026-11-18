# WP 2.7: mesh.dissolve_edges on grid loops: a crossing pair of loops, a chain ending at the
# boundary, use_face_split and an angle threshold that keeps the corner vertices.
# golden: startup=factory compare=objects,mesh:Grid,mesh:Grid.001,mesh:Grid.002 tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[2, 7, 12, 17, 22, 10, 11, 13, 14])
bpy.ops.mesh.dissolve_edges()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4, location=(3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[6, 7, 8])
bpy.ops.mesh.dissolve_edges(use_face_split=True)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4, location=(6, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[6, 7, 12, 11])
bpy.ops.mesh.dissolve_edges(angle_threshold=0.5)
bpy.ops.object.mode_set(mode='OBJECT')
