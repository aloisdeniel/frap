# WP 8.3.8: uv.select_all(INVERT/TOGGLE/SELECT/DESELECT) without sync selection in the edge and
# face UV select modes (tool_settings.uv_select_mode; its RNA update cleans the selection), on a
# corner-level selection made through the mesh-selected (visible) faces. Each step is dumped by
# renaming the UV selection attributes in Object Mode (Edit Mode then starts unselected).
# golden: startup=factory compare=mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.scene.tool_settings.use_uv_select_sync = False
bpy.ops.uv.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[1, 4])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.context.scene.tool_settings.uv_select_mode = 'EDGE'
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'edge_clean_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'edge_clean_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'edge_clean_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[1, 4])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.select_all(action='INVERT')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'edge_invert_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'edge_invert_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'edge_invert_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.scene.tool_settings.uv_select_mode = 'FACE'
bpy.ops.frap.select_elements(type='FACE', indices=[0, 2])
bpy.ops.uv.select_all(action='TOGGLE')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.select_all(action='INVERT')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'face_invert_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'face_invert_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'face_invert_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.uv.select_all(action='TOGGLE')
bpy.ops.uv.select_all(action='TOGGLE')
bpy.context.scene.tool_settings.uv_select_mode = 'VERTEX'
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'toggle_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'toggle_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'toggle_f'
