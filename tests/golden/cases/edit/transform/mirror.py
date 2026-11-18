# WP 2.12: transform.mirror reflects the selection about the pivot along the constrained axes
# of the orientation (one, two, three axes), nothing without constraint_axis.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1 tol=1e-5
import bpy
bpy.data.objects['Cube'].rotation_euler = (0, 0, 0.5)
bpy.data.objects['Cube'].scale = (1, 2, 1)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 1, 5])
bpy.ops.transform.mirror(constraint_axis=(True, False, False))
bpy.ops.transform.mirror(constraint_axis=(False, True, True), orient_type='LOCAL')
bpy.ops.transform.mirror(constraint_axis=(True, True, True), center_override=(0.5, 0, 0))
bpy.ops.transform.mirror()
bpy.context.scene.tool_settings.transform_pivot_point = 'CURSOR'
bpy.context.scene.cursor.location = (0, 1, 0)
bpy.ops.transform.mirror(constraint_axis=(False, True, False), orient_type='NORMAL')
