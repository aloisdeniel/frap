# WP 8.3.8: uv.select_linked with sync selection: from a mesh vertex selection the islands of
# the UVs at selected vertices are selected (sticky location); per-face UVs keep the faces of
# other islands at shared vertices partially selected in UV space, the mesh selection is the
# union. Face select mode with shared vertex selects mesh faces directly.
# golden: startup=factory compare=mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].uv_layers['UVMap'].data.foreach_set('uv', (0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0, 2.0, 0.0, 3.0, 0.0, 3.0, 1.0, 2.0, 1.0, 4.0, 0.0, 5.0, 0.0, 5.0, 1.0, 4.0, 1.0, 6.0, 0.0, 7.0, 0.0, 7.0, 1.0, 6.0, 1.0, 8.0, 0.0, 9.0, 0.0, 9.0, 1.0, 8.0, 1.0, 10.0, 0.0, 11.0, 0.0, 11.0, 1.0, 10.0, 1.0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0])
bpy.ops.uv.select_linked()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'vert_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'vert_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'vert_f'
bpy.data.meshes['Cube'].attributes['.select_vert'].name = 'vert_mv'
bpy.data.meshes['Cube'].attributes['.select_edge'].name = 'vert_me'
bpy.data.meshes['Cube'].attributes['.select_poly'].name = 'vert_mf'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.context.scene.tool_settings.uv_sticky_select_mode = 'SHARED_VERTEX'
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[0])
bpy.ops.uv.select_linked()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'face_vert_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'face_vert_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'face_vert_f'
bpy.data.meshes['Cube'].attributes['.select_vert'].name = 'face_vert_mv'
bpy.data.meshes['Cube'].attributes['.select_edge'].name = 'face_vert_me'
bpy.data.meshes['Cube'].attributes['.select_poly'].name = 'face_vert_mf'
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.scene.tool_settings.uv_sticky_select_mode = 'DISABLED'
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[2])
bpy.ops.uv.select_linked()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'face_dis_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'face_dis_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'face_dis_f'
bpy.data.meshes['Cube'].attributes['.select_vert'].name = 'face_dis_mv'
bpy.data.meshes['Cube'].attributes['.select_edge'].name = 'face_dis_me'
bpy.data.meshes['Cube'].attributes['.select_poly'].name = 'face_dis_mf'
