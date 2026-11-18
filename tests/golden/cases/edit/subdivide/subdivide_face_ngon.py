# WP 2.6: subdividing one face: the neighbours become n-gons (ngon=True), or are filled with
# triangles and quads (ngon=False, STRAIGHT_CUT acting as INNERVERT); one and two cuts.
# golden: startup=factory compare=objects,context,mesh:Cube.001,mesh:Cube.002,mesh:Cube.003 tier=T1
import bpy
bpy.ops.mesh.primitive_cube_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[5])
bpy.ops.mesh.subdivide()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[5])
bpy.ops.mesh.subdivide(ngon=False)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_cube_add(location=(-3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[5])
bpy.ops.mesh.subdivide(number_cuts=2)
