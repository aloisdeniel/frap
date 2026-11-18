# WP 2.6: mesh.unsubdivide reverses grid subdivision: a cube subdivided once comes back to 8
# vertices (default iterations=2 stops early); two subdivisions take two iterations; a partial
# selection only dissolves the selected candidates.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cube.001,mesh:Grid tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.subdivide()
bpy.ops.mesh.unsubdivide()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.subdivide(number_cuts=3)
bpy.ops.mesh.unsubdivide(iterations=1)
bpy.ops.mesh.unsubdivide(iterations=1)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=5, location=(0, 4, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1, 2, 5, 6, 7, 10, 11, 12])
bpy.ops.mesh.unsubdivide(iterations=3)
