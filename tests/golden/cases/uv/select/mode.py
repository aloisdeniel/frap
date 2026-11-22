# WP 8.3.8: uv.select_mode (headless-extended; Blender runs it in an Image Editor in UV mode)
# cleans the UV selection for the new mode under each sticky mode: vertex -> edge keeps only
# whole UV edges, edge -> face only whole faces, face -> vertex keeps the face corners; the same
# mode again is CANCELLED. The cube's UVs are split per face so sticky location and shared vertex
# differ.
# golden: startup=factory compare=mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].uv_layers['UVMap'].data.foreach_set('uv', (0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0, 2.0, 0.0, 3.0, 0.0, 3.0, 1.0, 2.0, 1.0, 4.0, 0.0, 5.0, 0.0, 5.0, 1.0, 4.0, 1.0, 6.0, 0.0, 7.0, 0.0, 7.0, 1.0, 6.0, 1.0, 8.0, 0.0, 9.0, 0.0, 9.0, 1.0, 8.0, 1.0, 10.0, 0.0, 11.0, 0.0, 11.0, 1.0, 10.0, 1.0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.scene.tool_settings.use_uv_select_sync = False
#@blender IE = {'window': bpy.context.window_manager.windows[0], 'area': [a for a in bpy.context.window_manager.windows[0].screen.areas if a.type == 'DOPESHEET_EDITOR'][0]}
#@blender IE['area'].type = 'IMAGE_EDITOR'
#@blender IE['area'].spaces.active.mode = 'UV'
#@blender IE['region'] = [r for r in IE['area'].regions if r.type == 'WINDOW'][0]
bpy.ops.uv.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[0])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[1, 4])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
#@frap bpy.ops.uv.select_mode(type='EDGE')
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.select_mode(type='EDGE') == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'loc_edge_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'loc_edge_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'loc_edge_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.scene.tool_settings.uv_sticky_select_mode = 'SHARED_VERTEX'
bpy.context.scene.tool_settings.uv_select_mode = 'VERTEX'
bpy.ops.frap.select_elements(type='FACE', indices=[2])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
#@frap bpy.ops.uv.select_mode(type='FACE')
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.select_mode(type='FACE') == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'vert_face_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'vert_face_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'vert_face_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.scene.tool_settings.uv_sticky_select_mode = 'DISABLED'
bpy.ops.frap.select_elements(type='FACE', indices=[3, 5])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
#@frap bpy.ops.uv.select_mode(type='VERTEX')
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.select_mode(type='VERTEX') == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'dis_vert_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'dis_vert_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'dis_vert_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.context.scene.tool_settings.uv_sticky_select_mode = 'SHARED_LOCATION'
#@frap bpy.ops.uv.select_mode(type='EDGE')
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.select_mode(type='EDGE') == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'loc_edge2_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'loc_edge2_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'loc_edge2_f'
