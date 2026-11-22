# WP 8.3.8: uv.select_more / uv.select_less (headless-extended) on a 4x4 quad grid without sync
# selection: vertex mode grows/shrinks by UV edges, face mode by faces; and with sync selection
# (sticky location: through the UV selection; shared vertex: mesh.select_more/less).
# golden: startup=empty compare=mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=5, y_subdivisions=5, size=2)
bpy.ops.object.mode_set(mode='EDIT')
#@blender IE = {'window': bpy.context.window_manager.windows[0], 'area': [a for a in bpy.context.window_manager.windows[0].screen.areas if a.type == 'DOPESHEET_EDITOR'][0]}
#@blender IE['area'].type = 'IMAGE_EDITOR'
#@blender IE['area'].spaces.active.mode = 'UV'
#@blender IE['region'] = [r for r in IE['area'].regions if r.type == 'WINDOW'][0]
bpy.context.scene.tool_settings.use_uv_select_sync = False
bpy.ops.uv.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[5])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
#@frap bpy.ops.uv.select_more()
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.select_more() == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.uv_select_vert'].name = 'v_more_v'
bpy.data.meshes['Grid'].attributes['.uv_select_edge'].name = 'v_more_e'
bpy.data.meshes['Grid'].attributes['.uv_select_face'].name = 'v_more_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1, 2, 4, 5, 6, 8, 9, 10])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
#@frap bpy.ops.uv.select_less()
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.select_less() == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.uv_select_vert'].name = 'v_less_v'
bpy.data.meshes['Grid'].attributes['.uv_select_edge'].name = 'v_less_e'
bpy.data.meshes['Grid'].attributes['.uv_select_face'].name = 'v_less_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.scene.tool_settings.uv_select_mode = 'FACE'
bpy.ops.frap.select_elements(type='FACE', indices=[5, 6])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
#@frap bpy.ops.uv.select_more()
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.select_more() == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.uv_select_vert'].name = 'f_more_v'
bpy.data.meshes['Grid'].attributes['.uv_select_edge'].name = 'f_more_e'
bpy.data.meshes['Grid'].attributes['.uv_select_face'].name = 'f_more_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1, 2, 4, 5, 6, 8, 9, 10])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
#@frap bpy.ops.uv.select_less()
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.select_less() == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.uv_select_vert'].name = 'f_less_v'
bpy.data.meshes['Grid'].attributes['.uv_select_edge'].name = 'f_less_e'
bpy.data.meshes['Grid'].attributes['.uv_select_face'].name = 'f_less_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.scene.tool_settings.use_uv_select_sync = True
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[12])
#@frap bpy.ops.uv.select_more()
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.select_more() == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.uv_select_vert'].name = 's_more_v'
bpy.data.meshes['Grid'].attributes['.uv_select_edge'].name = 's_more_e'
bpy.data.meshes['Grid'].attributes['.uv_select_face'].name = 's_more_f'
bpy.data.meshes['Grid'].attributes['.select_vert'].name = 's_more_mv'
bpy.data.meshes['Grid'].attributes['.select_edge'].name = 's_more_me'
bpy.data.meshes['Grid'].attributes['.select_poly'].name = 's_more_mf'
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.scene.tool_settings.uv_sticky_select_mode = 'SHARED_VERTEX'
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[6, 7, 8, 11, 12, 13, 16, 17, 18])
#@frap bpy.ops.uv.select_less()
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.select_less() == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.uv_select_vert'].name = 's_less_v'
bpy.data.meshes['Grid'].attributes['.uv_select_edge'].name = 's_less_e'
bpy.data.meshes['Grid'].attributes['.uv_select_face'].name = 's_less_f'
bpy.data.meshes['Grid'].attributes['.select_vert'].name = 's_less_mv'
bpy.data.meshes['Grid'].attributes['.select_edge'].name = 's_less_me'
bpy.data.meshes['Grid'].attributes['.select_poly'].name = 's_less_mf'
