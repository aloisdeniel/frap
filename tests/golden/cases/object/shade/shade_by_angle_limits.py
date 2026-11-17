# WP 1.21 (T1), object.shade_smooth_by_angle with angle pi (shade_smooth) and 0 (shade_flat).
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cube.001 tier=T1
import bpy
bpy.ops.object.shade_smooth_by_angle(angle=3.14159274)
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0))
bpy.ops.object.shade_smooth_by_angle(angle=0)
