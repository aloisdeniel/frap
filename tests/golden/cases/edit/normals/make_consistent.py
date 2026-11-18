# WP 2.9: mesh.normals_make_consistent: faces flipped inward are turned outward again (only
# selected faces flip); inside=True points every selected face inward. Two islands (a merged
# second cube) and an open grid are oriented independently.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.mesh.primitive_cube_add(size=1, location=(3, 0, 0))
bpy.ops.mesh.primitive_grid_add(x_subdivisions=3, y_subdivisions=3, location=(0, 3, 0))
bpy.ops.frap.select_elements(type='FACE', indices=[1, 5, 7, 12, 15])
bpy.ops.mesh.flip_normals()
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.normals_make_consistent()
bpy.ops.frap.select_elements(type='FACE', indices=[0, 2, 6, 8, 13])
bpy.ops.mesh.flip_normals()
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1, 2, 3, 4, 5, 6, 13])
bpy.ops.mesh.normals_make_consistent()
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.normals_make_consistent(inside=True)
