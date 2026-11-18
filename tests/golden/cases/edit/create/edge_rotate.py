# WP 2.8: mesh.edge_rotate rotates selected edges between two faces (a cube edge, both
# directions), several edges sharing faces (rotated longest first from the region boundary),
# and the errors for no selected edge and for edges that cannot rotate.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Grid tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.edge_rotate()
bpy.ops.frap.select_elements(type='EDGE', indices=[0])
bpy.ops.mesh.edge_rotate()
bpy.ops.mesh.edge_rotate(use_ccw=True)
bpy.data.meshes['Cube'].total_edge_sel
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.select_all(action='DESELECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=3, y_subdivisions=3, size=2, location=(4, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[0])
bpy.ops.mesh.edge_rotate()
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[3, 4, 7, 9])
bpy.ops.mesh.edge_rotate()
