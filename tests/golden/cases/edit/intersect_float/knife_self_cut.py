# WP 8.6: mesh.intersect(mode='SELECT', solver='FLOAT') with the default separate_mode CUT (ALL
# for self intersection).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T2 tol=1e-4
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.primitive_cube_add(size=1.2, location=(0.7, 0.4, 0.3), rotation=(0.3, 0.2, 0.1))
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.intersect(mode='SELECT', solver='FLOAT')
