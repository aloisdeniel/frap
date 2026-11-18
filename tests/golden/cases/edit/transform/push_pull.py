# WP 2.12: transform.push_pull pulls each vertex towards the pivot by an object-space
# distance (negative pushes away), with proportional editing and individual origins.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1 tol=1e-5
import bpy
bpy.data.objects['Cube'].rotation_euler = (0, 0.4, 0)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 1, 2, 3])
bpy.ops.transform.push_pull(value=0.3)
bpy.ops.transform.push_pull(value=-0.5, use_proportional_edit=True, proportional_size=2.5)
bpy.ops.transform.push_pull(value=0.2, center_override=(0, 0, 5))
bpy.context.scene.tool_settings.transform_pivot_point = 'INDIVIDUAL_ORIGINS'
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 3])
bpy.ops.transform.push_pull(value=0.1)
