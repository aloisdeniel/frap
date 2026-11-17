# WP 1.21 (T0), proportional rotation about individual origins with Light -> Cube (selected):
# the parent stays out of the proportional elements here too.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.ops.mesh.primitive_cube_add(location=(1.5, -1, 0))
bpy.data.objects['Light'].location = (1, 1, 1)
bpy.data.objects['Cube'].parent = bpy.data.objects['Light']
bpy.data.objects['Cube.001'].select_set(False)
bpy.data.objects['Cube'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.context.scene.tool_settings.transform_pivot_point = 'INDIVIDUAL_ORIGINS'
bpy.ops.transform.rotate(value=0.5, orient_axis='Z', use_proportional_edit=True, proportional_edit_falloff='LINEAR', proportional_size=20)
