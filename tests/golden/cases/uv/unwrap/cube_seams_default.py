# WP 3.8: unset uv.unwrap properties come from the tool settings and set ones are stored there:
# after unwrap(method='CONFORMAL', fill_holes=False) a bare unwrap() is CONFORMAL too, and a
# margin given once is reused by later calls (and by uv.pack_islands).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T2 tol=1e-4
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 2, 3, 5, 6, 9, 11])
bpy.ops.mesh.mark_seam()
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.unwrap(method='CONFORMAL', fill_holes=False)
bpy.ops.uv.unwrap()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.scene.cursor.location.x = bpy.data.meshes['Cube'].uv_layers.active.data[5].uv[0]
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.uv.unwrap(margin=0.1, margin_method='ADD')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.scene.cursor.location.y = bpy.data.meshes['Cube'].uv_layers.active.data[5].uv[0]
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.uv.unwrap(margin_method='ADD')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.scene.cursor.location.z = bpy.data.meshes['Cube'].uv_layers.active.data[5].uv[0]
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.uv.pack_islands(margin_method='ADD', rotate=False)
bpy.ops.object.mode_set(mode='OBJECT')
