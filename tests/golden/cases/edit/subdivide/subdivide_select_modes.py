# WP 2.6: mesh.subdivide of partial edge selections in edge and face select mode: one edge (no
# pattern: the faces gain corners), two adjacent edges (quad corner), three edges of a quad,
# opposite edges (connected straight), and two faces in face mode (selection flush after).
# golden: startup=factory compare=objects,context,mesh:Grid,mesh:Grid.001 tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[0])
bpy.ops.mesh.subdivide(number_cuts=2)
bpy.ops.frap.select_elements(type='EDGE', indices=[10, 11])
bpy.ops.mesh.subdivide(quadcorner='PATH')
bpy.ops.frap.select_elements(type='EDGE', indices=[20, 21, 22])
bpy.ops.mesh.subdivide()
bpy.ops.frap.select_elements(type='EDGE', indices=[30, 33])
bpy.ops.mesh.subdivide(number_cuts=3)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=3, y_subdivisions=3, location=(4, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 4])
bpy.ops.mesh.subdivide(number_cuts=2)
