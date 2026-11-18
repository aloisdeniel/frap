# WP 2.7: mesh.dissolve_verts merges the faces around each selected vertex into one n-gon;
# the cube's v0 gives V7 E9 F4 (research App. B.6); a boundary vertex between two edges
# joins them.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Grid tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[0])
bpy.ops.mesh.dissolve_verts()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4, location=(3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[2, 12, 18])
bpy.ops.mesh.dissolve_verts()
