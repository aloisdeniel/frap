# WP 8.6: mesh.intersect_boolean(operation='DIFFERENCE', solver='FLOAT') with a cutter whose three
# faces lie in the cube's face planes.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T2 tol=1e-4
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.primitive_cube_add(size=1, location=(0.5, 0.5, 0.5))
bpy.ops.mesh.intersect_boolean(operation='DIFFERENCE', solver='FLOAT')
