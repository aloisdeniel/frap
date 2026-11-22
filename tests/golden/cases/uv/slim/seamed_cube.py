# WP 8.1: uv.unwrap(method='MINIMUM_STRETCH') runs SLIM (D-109 closed: no "Built without
# SLIM" WARNING any more): the factory cube cut into the cross layout, the default 10
# iterations, then 3; the method and iterations are remembered by the tool settings for a
# bare uv.unwrap().
# golden: startup=factory compare=objects,context,mesh:Cube tier=T2 tol=1e-4
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 2, 3, 5, 6, 9, 11])
bpy.ops.mesh.mark_seam()
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.unwrap(method='MINIMUM_STRETCH')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.scene.cursor.location.x = bpy.data.meshes['Cube'].uv_layers.active.data[5].uv[0]
bpy.context.scene.cursor.location.y = bpy.data.meshes['Cube'].uv_layers.active.data[5].uv[1]
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.uv.unwrap(method='MINIMUM_STRETCH', iterations=3)
bpy.ops.uv.unwrap()
bpy.ops.object.mode_set(mode='OBJECT')
