# WP 8.3.1: mesh.select_boundary_loop_multi on a grid with a hole: one selected boundary edge
# of the outer ring and one of the hole select both whole boundaries (extend keeps the other
# selected edges), without extend only the loops remain, SEAM delimiters stop the loop, a
# selection without boundary edges is Blender's error, and a hidden part of the boundary is
# not walked.
# golden: startup=empty compare=mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=6)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[14, 15, 20, 21])
bpy.ops.mesh.delete(type='FACE')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_boundary_loop_multi()
bpy.ops.frap.select_elements(type='EDGE', indices=[1, 40, 42])
bpy.ops.mesh.select_boundary_loop_multi()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.select_edge'].name = 'extend'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='EDGE', indices=[1, 40, 42])
bpy.ops.mesh.select_boundary_loop_multi(extend=False)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.select_edge'].name = 'replace'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 4])
bpy.ops.mesh.mark_seam()
bpy.ops.frap.select_elements(type='EDGE', indices=[1])
bpy.ops.mesh.select_boundary_loop_multi(delimit_edge_loop={'SEAM'})
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.select_edge'].name = 'seam'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='EDGE', indices=[13])
bpy.ops.mesh.hide(unselected=False)
bpy.ops.frap.select_elements(type='EDGE', indices=[1])
bpy.ops.mesh.select_boundary_loop_multi(extend=False)
