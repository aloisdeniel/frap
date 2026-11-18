# WP 4.1 (T1): object.shade_smooth and shade_flat remove the "Smooth by Angle" modifier of the
# selected mesh objects; shade_smooth_by_angle keeps it.
# golden: startup=factory compare=objects,mesh:Cube,mesh:Cylinder,evaluated:Cylinder tier=T1
import bpy
bpy.ops.object.shade_auto_smooth()
bpy.ops.object.shade_smooth_by_angle()
bpy.ops.object.shade_smooth()
bpy.ops.mesh.primitive_cylinder_add(vertices=8, location=(3, 0, 0))
bpy.ops.object.shade_auto_smooth(angle=1.0)
bpy.data.objects['Cylinder'].modifiers.new('GN', 'NODES')
bpy.ops.object.shade_flat()
bpy.ops.object.shade_auto_smooth(angle=1.0)
