# WP 8.3.8: uv.hide / uv.reveal. Without sync selection hiding deselects the faces with selected
# UVs in the mesh (vertex mode: their vertices, two passes), unselected hides the others, reveal
# brings the unselected mesh faces back with select True/False; with sync selection they are
# mesh.hide / mesh.reveal.
# golden: startup=factory compare=mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.scene.tool_settings.use_uv_select_sync = False
bpy.ops.uv.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[4])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.hide()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'hide_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'hide_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'hide_f'
bpy.data.meshes['Cube'].attributes['.select_vert'].name = 'hide_mv'
bpy.data.meshes['Cube'].attributes['.select_edge'].name = 'hide_me'
bpy.data.meshes['Cube'].attributes['.select_poly'].name = 'hide_mf'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1, 2])
bpy.ops.uv.reveal()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'reveal_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'reveal_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'reveal_f'
bpy.data.meshes['Cube'].attributes['.select_vert'].name = 'reveal_mv'
bpy.data.meshes['Cube'].attributes['.select_edge'].name = 'reveal_me'
bpy.data.meshes['Cube'].attributes['.select_poly'].name = 'reveal_mf'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[3])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.hide(unselected=True)
bpy.ops.uv.reveal(select=False)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'unsel_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'unsel_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'unsel_f'
bpy.data.meshes['Cube'].attributes['.select_vert'].name = 'unsel_mv'
bpy.data.meshes['Cube'].attributes['.select_edge'].name = 'unsel_me'
bpy.data.meshes['Cube'].attributes['.select_poly'].name = 'unsel_mf'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.context.scene.tool_settings.uv_select_mode = 'FACE'
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 5])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.hide()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'face_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'face_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'face_f'
bpy.data.meshes['Cube'].attributes['.select_vert'].name = 'face_mv'
bpy.data.meshes['Cube'].attributes['.select_edge'].name = 'face_me'
bpy.data.meshes['Cube'].attributes['.select_poly'].name = 'face_mf'
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.scene.tool_settings.use_uv_select_sync = True
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[1])
bpy.ops.uv.hide()
bpy.ops.uv.hide(unselected=True)
bpy.ops.uv.reveal()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'sync_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'sync_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'sync_f'
bpy.data.meshes['Cube'].attributes['.select_vert'].name = 'sync_mv'
bpy.data.meshes['Cube'].attributes['.select_edge'].name = 'sync_me'
bpy.data.meshes['Cube'].attributes['.select_poly'].name = 'sync_mf'
