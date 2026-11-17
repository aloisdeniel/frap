# WP 1.21 (T1), object.transform_apply: location, rotation and scale (deltas included) go into
# the mesh and are reset; the child keeps its world matrix through Blender's stale-delta parent
# matrix (it moves because the parent had deltas, as in Blender).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.data.objects['Light'].parent = bpy.data.objects['Cube']
bpy.data.objects['Cube'].location = (1, 2, 3)
bpy.data.objects['Cube'].rotation_euler = (0.3, 0, 0.5)
bpy.data.objects['Cube'].scale = (2, 1, 0.5)
bpy.data.objects['Cube'].delta_location = (0.5, 0, 0)
bpy.data.objects['Cube'].delta_scale = (1, 2, 1)
bpy.ops.object.transform_apply()
bpy.data.objects['Light'].matrix_parent_inverse
