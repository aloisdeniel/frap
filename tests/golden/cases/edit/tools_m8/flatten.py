# WP 8.3.3: mesh.flatten (new in 5.x): a jittered grid flattened on its best-fit plane, two
# separate face regions (one plane each) with NORMAL and a factor, locked axes, a loose
# selected edge chain (best fit), VIEW in the virtual 3D View on a rotated object, and a
# selection of vertices only (nothing to flatten).
# golden: startup=factory compare=objects,context,mesh:Grid,mesh:Cube tier=T1
import bpy
bpy.ops.object.delete()
bpy.ops.mesh.primitive_grid_add(x_subdivisions=7, y_subdivisions=7, size=2)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.transform.vertex_random(offset=0.3, seed=11)
bpy.ops.mesh.flatten()
bpy.ops.transform.vertex_random(offset=0.3, seed=12)
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1, 6, 7, 28, 29, 34, 35])
bpy.ops.mesh.flatten(method='NORMAL', factor=0.7)
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[14, 15, 16, 20, 21])
bpy.ops.mesh.flatten(lock=(False, True, False))
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[40, 41, 43, 45, 47])
bpy.ops.mesh.flatten(method='NORMAL')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[3, 30, 60])
bpy.ops.mesh.flatten()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_cube_add(location=(4, 0, 0), rotation=(0.4, 0.1, 0.3))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.flatten(method='VIEW', factor=0.5)
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[1, 2])
bpy.ops.mesh.flatten(method='VIEW', lock=(True, False, True))
