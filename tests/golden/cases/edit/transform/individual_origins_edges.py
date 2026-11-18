# WP 2.12: INDIVIDUAL_ORIGINS in vertex/edge select mode: islands of selected edges connected
# through vertices, a lone selected vertex stays in place.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1 tol=1e-5
import bpy
bpy.data.objects['Cube'].scale = (1, 2, 1)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='EDGE', indices=[2, 9])
bpy.ops.frap.select_elements(type='VERT', indices=[7], action='ADD')
bpy.context.scene.tool_settings.transform_pivot_point = 'INDIVIDUAL_ORIGINS'
bpy.ops.transform.resize(value=(0.5, 0.5, 0.5))
bpy.ops.transform.rotate(value=0.8, orient_axis='Y')
bpy.ops.transform.translate(value=(0.2, 0, 0), orient_type='NORMAL')
