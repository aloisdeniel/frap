# WP 2.12: proportional editing with INDIVIDUAL_ORIGINS: unselected vertices take the center
# and axes of the nearest selected one; connected distances map vertices to the island they were
# reached from (single-vertex islands in vertex select mode).
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1 tol=1e-5
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=8, y_subdivisions=8, size=4)
bpy.context.scene.tool_settings.transform_pivot_point = 'INDIVIDUAL_ORIGINS'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='EDGE', indices=[10, 30])
bpy.ops.transform.resize(value=(0.5, 0.5, 1), use_proportional_edit=True, proportional_size=1.5)
bpy.ops.transform.rotate(value=0.5, orient_axis='Z', use_proportional_edit=True, use_proportional_connected=True, proportional_size=1.2)
bpy.ops.frap.select_elements(type='VERT', indices=[40, 52])
bpy.ops.transform.resize(value=(2, 2, 2), use_proportional_edit=True, use_proportional_connected=True, proportional_size=1.0)
bpy.ops.transform.translate(value=(0, 0, 0.5), orient_type='NORMAL', constraint_axis=(False, False, True), use_proportional_edit=True, proportional_size=1.0)
