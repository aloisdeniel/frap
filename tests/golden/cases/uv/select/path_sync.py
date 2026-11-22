# WP 8.3.8: uv.shortest_path_select with sync selection and shared vertex (the UV selection is
# the mesh selection): a vertex path toggles mesh vertices.
# golden: startup=empty compare=mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=5, y_subdivisions=5, size=2)
bpy.ops.object.mode_set(mode='EDIT')
#@blender IE = {'window': bpy.context.window_manager.windows[0], 'area': [a for a in bpy.context.window_manager.windows[0].screen.areas if a.type == 'DOPESHEET_EDITOR'][0]}
#@blender IE['area'].type = 'IMAGE_EDITOR'
#@blender IE['area'].spaces.active.mode = 'UV'
#@blender IE['region'] = [r for r in IE['area'].regions if r.type == 'WINDOW'][0]
bpy.context.scene.tool_settings.uv_sticky_select_mode = 'SHARED_VERTEX'
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 24])
#@frap bpy.ops.uv.shortest_path_select()
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.shortest_path_select() == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.uv_select_vert'].name = 'vert_v'
bpy.data.meshes['Grid'].attributes['.uv_select_edge'].name = 'vert_e'
bpy.data.meshes['Grid'].attributes['.uv_select_face'].name = 'vert_f'
bpy.data.meshes['Grid'].attributes['.select_vert'].name = 'vert_mv'
bpy.data.meshes['Grid'].attributes['.select_edge'].name = 'vert_me'
bpy.data.meshes['Grid'].attributes['.select_poly'].name = 'vert_mf'
