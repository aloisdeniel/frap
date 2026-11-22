# WP 8.3.5 (T1), transform.shear in Object Mode with every pivot point: only the locations
# change, sheared about the pivot (each origin for INDIVIDUAL_ORIGINS: nothing moves).
# golden: startup=factory compare=objects,context tier=T1 tol=1e-5
import bpy
bpy.data.objects['Light'].select_set(True)
bpy.data.objects['Camera'].select_set(True)
bpy.context.scene.cursor.location = (1, -2, 0.5)
bpy.ops.transform.shear(angle=0.4)
bpy.context.scene.tool_settings.transform_pivot_point = 'CURSOR'
bpy.ops.transform.shear(angle=0.3, orient_axis='X', orient_axis_ortho='Y')
bpy.context.scene.tool_settings.transform_pivot_point = 'BOUNDING_BOX_CENTER'
bpy.ops.transform.shear(angle=-0.2, orient_axis='Y', orient_axis_ortho='Z')
bpy.context.scene.tool_settings.transform_pivot_point = 'ACTIVE_ELEMENT'
bpy.ops.transform.shear(angle=0.5, orient_axis='Z', orient_axis_ortho='Y')
bpy.context.scene.tool_settings.transform_pivot_point = 'INDIVIDUAL_ORIGINS'
bpy.ops.transform.shear(angle=0.7)
bpy.context.scene.tool_settings.transform_pivot_point = 'MEDIAN_POINT'
bpy.ops.transform.shear(angle=0.25, orient_axis='X')
