# WP 8.3.8: uv.shortest_path_select (headless-extended) with nth + skip wrapping to INT_MIN
# (nth=1, skip=2147483647, offset=-5, inside the RNA ranges): Blender's mod_i wraps like C
# ints and finishes.
# golden: startup=empty compare=mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=6, size=2)
bpy.ops.object.mode_set(mode='EDIT')
#@blender IE = {'window': bpy.context.window_manager.windows[0], 'area': [a for a in bpy.context.window_manager.windows[0].screen.areas if a.type == 'DOPESHEET_EDITOR'][0]}
#@blender IE['area'].type = 'IMAGE_EDITOR'
#@blender IE['area'].spaces.active.mode = 'UV'
#@blender IE['region'] = [r for r in IE['area'].regions if r.type == 'WINDOW'][0]
bpy.context.scene.tool_settings.use_uv_select_sync = False
bpy.ops.uv.select_all(action='DESELECT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 35])
bpy.context.scene.tool_settings.use_uv_select_sync = True
bpy.context.scene.tool_settings.use_uv_select_sync = False
bpy.ops.mesh.select_all(action='SELECT')
#@frap bpy.ops.uv.shortest_path_select(nth=1, skip=2147483647, offset=-5)
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.shortest_path_select(nth=1, skip=2147483647, offset=-5) == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.uv_select_vert'].name = 'a_v'
bpy.data.meshes['Grid'].attributes['.uv_select_edge'].name = 'a_e'
bpy.data.meshes['Grid'].attributes['.uv_select_face'].name = 'a_f'
