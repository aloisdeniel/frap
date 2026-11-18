# WP 2.12: shrink_fatten with INDIVIDUAL_ORIGINS (normals from createSpaceNormal), in edge
# select mode, and through transform.transform with proportional editing.
# golden: startup=factory compare=objects,context,mesh:Cylinder tier=T1 tol=1e-5
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=8, rotation=(0.3, 0.2, 0))
bpy.context.scene.tool_settings.transform_pivot_point = 'INDIVIDUAL_ORIGINS'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[3, 4])
bpy.ops.transform.shrink_fatten(value=0.2, use_even_offset=True)
bpy.ops.transform.transform(mode='SHRINKFATTEN', value=(-0.1, 0, 0, 0), use_proportional_edit=True, proportional_size=0.8)
