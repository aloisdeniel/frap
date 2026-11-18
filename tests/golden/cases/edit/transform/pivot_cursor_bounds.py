# WP 2.12: the 3D cursor and bounding-box pivots, and center_override.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1 tol=1e-5
import bpy
bpy.data.objects['Cube'].location = (0.5, 0, 0)
bpy.data.objects['Cube'].rotation_euler = (0, 0, 0.6)
bpy.context.scene.cursor.location = (1, 1, 1)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 2, 3, 7])
bpy.context.scene.tool_settings.transform_pivot_point = 'CURSOR'
bpy.ops.transform.resize(value=(0.5, 0.5, 0.5))
bpy.ops.transform.rotate(value=0.7, orient_axis='X')
bpy.context.scene.tool_settings.transform_pivot_point = 'BOUNDING_BOX_CENTER'
bpy.ops.transform.resize(value=(2, 1, 1))
bpy.ops.transform.rotate(value=-0.5, orient_axis='Z', center_override=(0, 0, 3))
