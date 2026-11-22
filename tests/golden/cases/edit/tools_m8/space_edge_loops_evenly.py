# WP 8.3.3: mesh.space_edge_loops_evenly (new in 5.x): a closed circle with uneven spacing
# (periodic cubic spline) and LINEAR, an open edge row of a jittered grid (natural spline, the
# ends stay), a factor and locked axes, a junction (three selected edges at a vertex ends the
# chains); one selected edge: WARNING "No edge loops found …"; nothing selected: WARNING
# "No edges selected".
# golden: startup=factory compare=objects,context,mesh:Circle,mesh:Grid tier=T1
import bpy
bpy.ops.object.delete()
bpy.ops.mesh.primitive_circle_add(vertices=12, radius=1.5)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[1, 2, 7])
bpy.ops.transform.translate(value=(0.3, 0.2, 0.1))
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.space_edge_loops_evenly(factor=0.5)
bpy.ops.transform.vertex_random(offset=0.2, seed=8)
bpy.ops.mesh.space_edge_loops_evenly(interpolation='LINEAR')
bpy.ops.transform.vertex_random(offset=0.2, seed=9)
bpy.ops.mesh.space_edge_loops_evenly(lock=(False, False, True))
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=4, size=3, location=(4, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.transform.vertex_random(offset=0.25, seed=21)
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 3, 6, 9, 12])
bpy.ops.mesh.space_edge_loops_evenly()
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[16, 19, 22, 25, 28, 17, 30])
bpy.ops.mesh.space_edge_loops_evenly(interpolation='LINEAR', factor=0.8)
bpy.ops.mesh.space_edge_loops_evenly(interpolation='CUBIC', lock=(True, False, False))
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[40])
bpy.ops.mesh.space_edge_loops_evenly()
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.space_edge_loops_evenly()
