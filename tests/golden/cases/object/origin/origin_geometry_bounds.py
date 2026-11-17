# WP 1.21 (T1), object.origin_set(ORIGIN_GEOMETRY): center BOUNDS, and an unset center following
# the BOUNDING_BOX_CENTER pivot.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.data.meshes['Cube'].vertices[0].co = (3, 3, 3)
bpy.ops.object.origin_set(type='ORIGIN_GEOMETRY', center='BOUNDS')
bpy.data.meshes['Cube'].vertices[1].co = (1, 1, -5)
bpy.context.scene.tool_settings.transform_pivot_point = 'BOUNDING_BOX_CENTER'
bpy.ops.object.origin_set(type='ORIGIN_GEOMETRY')
