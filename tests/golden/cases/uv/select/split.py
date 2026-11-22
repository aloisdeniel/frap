# WP 8.3.8: uv.select_split keeps only the entirely selected UV faces (mixed faces lose their
# UV selection); CANCELLED when no face is mixed; with sync selection only the UV selection
# changes, and shared vertex is refused.
# golden: startup=factory compare=mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.scene.tool_settings.use_uv_select_sync = False
bpy.ops.uv.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[4])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.context.scene.tool_settings.uv_select_mode = 'VERTEX'
bpy.ops.uv.select_split()
bpy.ops.uv.select_split()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'nosync_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'nosync_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'nosync_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.scene.tool_settings.use_uv_select_sync = True
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[0])
bpy.ops.uv.select_split()
bpy.context.scene.tool_settings.uv_sticky_select_mode = 'SHARED_VERTEX'
bpy.ops.uv.select_split()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'sync_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'sync_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'sync_f'
bpy.data.meshes['Cube'].attributes['.select_vert'].name = 'sync_mv'
bpy.data.meshes['Cube'].attributes['.select_edge'].name = 'sync_me'
bpy.data.meshes['Cube'].attributes['.select_poly'].name = 'sync_mf'
