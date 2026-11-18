# WP 2.7: mesh.dissolve_verts(use_face_split=True) cuts the corners off first, and
# use_boundary_tear=True splits boundary vertices off instead of merging across them.
# golden: startup=factory compare=objects,mesh:Grid,mesh:Grid.001 tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[6, 12])
bpy.ops.mesh.dissolve_verts(use_face_split=True)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4, location=(3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[2, 7, 12])
bpy.ops.mesh.dissolve_verts(use_boundary_tear=True)
bpy.ops.object.mode_set(mode='OBJECT')
