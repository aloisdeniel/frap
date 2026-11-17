# WP 1.21 (T1), object.shade_smooth_by_angle on an ico sphere and a cylinder (rims sharp, sides
# smooth).
# golden: startup=factory compare=objects,context,mesh:Icosphere,mesh:Cylinder tier=T1
import bpy
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=1)
bpy.ops.object.shade_smooth_by_angle(angle=0.6)
bpy.ops.mesh.primitive_cylinder_add(vertices=12, location=(3, 0, 0))
bpy.ops.object.shade_smooth_by_angle(angle=0.6)
