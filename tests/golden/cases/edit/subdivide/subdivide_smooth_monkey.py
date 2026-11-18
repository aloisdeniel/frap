# WP 2.6: mesh.subdivide smoothness on a curved mesh (Suzanne, partial edge selection, PATH
# corners): the dual-sphere interpolation must match Blender's float chain at the default tol.
# golden: startup=factory compare=objects,context,mesh:Suzanne tier=T1
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[3, 4, 5, 10, 50, 51, 52, 53, 200, 201])
bpy.ops.mesh.subdivide(number_cuts=3, smoothness=0.7, quadcorner='PATH')
