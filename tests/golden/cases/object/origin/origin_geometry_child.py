# WP 1.21 (T1), object.origin_set(ORIGIN_GEOMETRY) on a rotated, scaled parent: the origin moves
# to the median, nothing moves in world space, the child keeps its world matrix.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.data.meshes['Cube'].vertices[0].co = (3, 3, 3)
bpy.data.objects['Light'].parent = bpy.data.objects['Cube']
bpy.data.objects['Cube'].location = (1, 0, 0)
bpy.data.objects['Cube'].rotation_euler = (0, 0, 0.5)
bpy.data.objects['Cube'].scale = (2, 1, 1)
bpy.ops.object.origin_set(type='ORIGIN_GEOMETRY')
