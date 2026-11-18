# WP 2.12: INDIVIDUAL_ORIGINS in face select mode: each island of faces sharing an edge scales
# and rotates about its own center; with orient_type NORMAL each island uses its own axes.
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1 tol=1e-5
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=6, size=4)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1, 14, 20, 21, 35])
bpy.context.scene.tool_settings.transform_pivot_point = 'INDIVIDUAL_ORIGINS'
bpy.ops.transform.resize(value=(0.5, 0.7, 1))
bpy.ops.transform.rotate(value=0.5, orient_axis='Z')
bpy.ops.transform.translate(value=(0, 0, 0.4))
bpy.ops.transform.resize(value=(1, 1, 0.5), orient_type='NORMAL', constraint_axis=(False, False, True))
bpy.ops.transform.rotate(value=0.3, orient_axis='X', orient_type='NORMAL', constraint_axis=(True, False, False))
