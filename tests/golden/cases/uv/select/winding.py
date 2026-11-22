# WP 8.3.8: uv.select_by_winding selects the UV faces by the sign of their UV area: NEGATIVE
# (clockwise, faces 1 and 3 here), POSITIVE, extend; a degenerate face (5) is neither. Without
# and with sync selection.
# golden: startup=factory compare=mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].uv_layers['UVMap'].data.foreach_set('uv', (0, 0, 1, 0, 1, 1, 0, 1, 2, 0, 2, 1, 3, 1, 3, 0, 4, 0, 5, 0, 5, 1, 4, 1, 6, 0, 6, 1, 7, 1, 7, 0, 8, 0, 9, 0, 9, 1, 8, 1, 10, 0, 11, 0, 10.5, 0, 10.5, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.scene.tool_settings.use_uv_select_sync = False
bpy.ops.uv.select_by_winding()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'neg_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'neg_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'neg_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.uv.select_by_winding(winding='POSITIVE')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'pos_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'pos_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'pos_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.uv.select_by_winding(winding='NEGATIVE')
bpy.ops.uv.select_by_winding(winding='POSITIVE', extend=True)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'ext_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'ext_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'ext_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.scene.tool_settings.use_uv_select_sync = True
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.uv.select_by_winding()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'sync_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'sync_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'sync_f'
bpy.data.meshes['Cube'].attributes['.select_vert'].name = 'sync_mv'
bpy.data.meshes['Cube'].attributes['.select_edge'].name = 'sync_me'
bpy.data.meshes['Cube'].attributes['.select_poly'].name = 'sync_mf'
