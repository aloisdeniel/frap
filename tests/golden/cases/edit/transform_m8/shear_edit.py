# WP 8.3.5 (T1), transform.shear in Edit Mode on a rotated and scaled cube: every pivot
# (individual origins: each face island about its center), the NORMAL, LOCAL, VIEW and CURSOR
# orientations, proportional editing (connected and not); two equal axes collapse two vertices
# onto their median (no face collapses: a zero-area face's normal is rounding noise).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1 tol=1e-5
import bpy
bpy.data.objects['Cube'].rotation_euler = (0.3, 0.2, 0.1)
bpy.data.objects['Cube'].scale = (1, 1.5, 0.8)
bpy.data.objects['Cube'].location = (0.5, 0, 1)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.subdivide(number_cuts=2)
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1, 2, 30, 31])
bpy.ops.transform.shear(angle=0.3)
bpy.ops.transform.shear(angle=0.2, orient_type='NORMAL', orient_axis='X', orient_axis_ortho='Z')
bpy.ops.transform.shear(angle=-0.25, orient_type='LOCAL', orient_axis='Y', orient_axis_ortho='X')
bpy.ops.transform.shear(angle=0.15, orient_type='VIEW')
bpy.context.scene.cursor.rotation_euler = (0.4, 0, 0.7)
bpy.ops.transform.shear(angle=0.3, orient_type='CURSOR', orient_axis='X', orient_axis_ortho='Y')
bpy.context.scene.tool_settings.transform_pivot_point = 'INDIVIDUAL_ORIGINS'
bpy.ops.transform.shear(angle=0.4)
bpy.context.scene.tool_settings.transform_pivot_point = 'CURSOR'
bpy.context.scene.cursor.location = (1, 1, 1)
bpy.ops.transform.shear(angle=0.2, orient_axis='Y', orient_axis_ortho='Z')
bpy.context.scene.tool_settings.transform_pivot_point = 'BOUNDING_BOX_CENTER'
bpy.ops.transform.shear(angle=0.3, use_proportional_edit=True, proportional_size=1.5)
bpy.context.scene.tool_settings.transform_pivot_point = 'ACTIVE_ELEMENT'
bpy.ops.transform.shear(angle=-0.3, use_proportional_edit=True, use_proportional_connected=True, proportional_edit_falloff='SPHERE', proportional_size=2)
bpy.context.scene.tool_settings.transform_pivot_point = 'MEDIAN_POINT'
bpy.ops.transform.shear(angle=0.2, orient_type='GIMBAL', orient_axis='X', orient_axis_ortho='Y')
bpy.ops.transform.shear(angle=0.1, orient_type='PARENT')
bpy.ops.transform.shear(angle=0.2, orient_matrix=((0, 1, 0), (-1, 0, 0), (0, 0, 1)), orient_matrix_type='GLOBAL')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 5])
bpy.ops.transform.shear(angle=0.25, orient_axis='Z', orient_axis_ortho='Z')
bpy.ops.frap.select_elements(type='VERT', indices=[])
bpy.ops.transform.shear(angle=0.3)
