# WP 8.3.5 (T0), transform.bend has no exec in Blender 5.2.4: a script call changes nothing
# (PASS_THROUGH) in Object Mode and in Edit Mode, with every pivot point.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T0
import bpy
bpy.data.objects['Light'].select_set(True)
bpy.ops.transform.bend(value=(0.8,))
bpy.context.scene.tool_settings.transform_pivot_point = 'CURSOR'
bpy.ops.transform.bend(value=(-1.2,), use_proportional_edit=True)
bpy.context.scene.tool_settings.transform_pivot_point = 'INDIVIDUAL_ORIGINS'
bpy.ops.transform.bend(value=(0.5,), center_override=(1, 2, 3))
bpy.data.objects['Light'].select_set(False)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.transform.bend(value=(0.6,))
bpy.context.scene.tool_settings.transform_pivot_point = 'MEDIAN_POINT'
bpy.ops.transform.bend(value=(3.0,))
