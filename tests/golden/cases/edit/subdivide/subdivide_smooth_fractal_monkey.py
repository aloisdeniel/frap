# WP 2.6: mesh.subdivide smoothness plus fractal on Suzanne (partial edge selection, PATH
# corners): smoothing followed by the seeded fractal displacement, at the default tol.
# golden: startup=factory compare=objects,context,mesh:Suzanne tier=T1
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[3, 4, 5, 10, 50, 51, 52, 53, 200, 201])
bpy.ops.mesh.subdivide(number_cuts=3, smoothness=0.7, fractal=3.0, seed=11, quadcorner='PATH')
