# WP 8.6: mesh.intersect_boolean(operation='UNION', solver='FLOAT'): a rotated cube added in Edit
# Mode (selected) and the default cube (unselected).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T2 tol=1e-4
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.primitive_cube_add(size=1.2, location=(0.7, 0.4, 0.3), rotation=(0.3, 0.2, 0.1))
bpy.ops.mesh.intersect_boolean(operation='UNION', solver='FLOAT')
