# WP 2.6: mesh.subdivide_edgering on the cube's vertical ring (open rims): the default (10 cuts,
# PATH), LINEAR with 2 cuts, SURFACE with a profile; a single edge is no ring (INFO report,
# unchanged).
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cube.001,mesh:Cube.002,mesh:Cube.003 tier=T1 tol=1e-4
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 2, 7, 8])
bpy.ops.mesh.subdivide_edgering()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[1, 3, 6, 9])
bpy.ops.mesh.subdivide_edgering(number_cuts=2, interpolation='LINEAR')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_cube_add(location=(-3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[4, 5, 10, 11])
bpy.ops.mesh.subdivide_edgering(number_cuts=3, interpolation='SURFACE', profile_shape_factor=0.5, profile_shape='SPHERE')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_cube_add(location=(0, 3, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[4])
bpy.ops.mesh.subdivide_edgering(number_cuts=2)
