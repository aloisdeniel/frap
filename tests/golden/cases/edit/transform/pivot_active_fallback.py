# WP 2.12: pivot ACTIVE_ELEMENT without a selection history falls back to the median point;
# the active element is taken from the first edit object with something selected.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cube.001 tier=T1 tol=1e-5
import bpy
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0), size=1)
bpy.data.objects['Cube'].select_set(True)
bpy.context.scene.tool_settings.transform_pivot_point = 'ACTIVE_ELEMENT'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.transform.resize(value=(0.5, 0.5, 0.5))
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[2, 6], object='Cube')
bpy.ops.transform.rotate(value=0.5, orient_axis='Z')
bpy.ops.transform.translate(value=(0, 0, 1), orient_type='NORMAL')
