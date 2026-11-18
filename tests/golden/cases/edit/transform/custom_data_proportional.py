# WP 2.12: crease and bevel weights through transform.transform with proportional editing
# (edge centers measured in world space, connected included) and with INDIVIDUAL_ORIGINS.
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1 tol=1e-5
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=5, y_subdivisions=5, size=4)
bpy.data.objects['Grid'].scale = (1, 2, 1)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='EDGE', indices=[12])
bpy.ops.transform.transform(mode='CREASE', value=(0.8, 0, 0, 0), use_proportional_edit=True, proportional_size=2)
bpy.ops.transform.transform(mode='BWEIGHT', value=(0.6, 0, 0, 0), use_proportional_edit=True, use_proportional_connected=True, proportional_size=1.5, proportional_edit_falloff='LINEAR')
bpy.ops.transform.transform(mode='VERTEX_CREASE', value=(0.7, 0, 0, 0), use_proportional_edit=True, proportional_size=1.8, proportional_edit_falloff='SHARP')
bpy.context.scene.tool_settings.transform_pivot_point = 'INDIVIDUAL_ORIGINS'
bpy.ops.transform.transform(mode='CREASE', value=(-0.3, 0, 0, 0), use_proportional_edit=True, proportional_size=1.0)
bpy.ops.transform.transform(mode='VERTEX_CREASE', value=(0.2, 0, 0, 0), use_proportional_edit=True, use_proportional_connected=True)
