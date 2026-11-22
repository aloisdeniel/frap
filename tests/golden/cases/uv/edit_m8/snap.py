# WP 8.3.9: uv.snap_cursor / uv.snap_selected / uv.move_on_axis (headless-extended: frap's
# virtual Image Editor; Blender runs them in an Image Editor in UV mode) on a jittered grid:
# the 2D cursor to the selection's bounds centre, to the pixel grid (256 x 256 without an
# image) and to the origin; the selection to the cursor, by the cursor offset, to pixels and
# to the adjacent unselected UVs (no sync selection); move_on_axis by UDIM tiles, pixels and
# custom-grid steps; arrange_islands from the cursor.
# golden: startup=empty compare=mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=5, y_subdivisions=5, size=2)
bpy.data.meshes['Grid'].uv_layers['UVMap'].data.foreach_set('uv', (-0.014, -0.028, 0.212, -0.034, 0.17, 0.178, -0.006, 0.226, 0.212, -0.034, 0.403, -0.011, 0.41, 0.236, 0.17, 0.178, 0.403, -0.011, 0.565, 0.001, 0.606, 0.192, 0.41, 0.236, 0.565, 0.001, 0.763, -0.005, 0.838, 0.164, 0.606, 0.192, 0.763, -0.005, 0.966, -0.033, 1.029, 0.183, 0.838, 0.164, -0.006, 0.226, 0.17, 0.178, 0.185, 0.425, -0.028, 0.369, 0.17, 0.178, 0.41, 0.236, 0.374, 0.407, 0.185, 0.425, 0.41, 0.236, 0.606, 0.192, 0.611, 0.39, 0.374, 0.407, 0.606, 0.192, 0.838, 0.164, 0.804, 0.365, 0.611, 0.39, 0.838, 0.164, 1.029, 0.183, 0.965, 0.376, 0.804, 0.365, -0.028, 0.369, 0.185, 0.425, 0.185, 0.607, 0.014, 0.594, 0.185, 0.425, 0.374, 0.407, 0.396, 0.584, 0.185, 0.607, 0.374, 0.407, 0.611, 0.39, 0.624, 0.616, 0.396, 0.584, 0.611, 0.39, 0.804, 0.365, 0.78, 0.606, 0.624, 0.616, 0.804, 0.365, 0.965, 0.376, 1.002, 0.63, 0.78, 0.606, 0.014, 0.594, 0.185, 0.607, 0.238, 0.769, 0.018, 0.783, 0.185, 0.607, 0.396, 0.584, 0.393, 0.821, 0.238, 0.769, 0.396, 0.584, 0.624, 0.616, 0.572, 0.799, 0.393, 0.821, 0.624, 0.616, 0.78, 0.606, 0.763, 0.813, 0.572, 0.799, 0.78, 0.606, 1.002, 0.63, 1.021, 0.806, 0.763, 0.813, 0.018, 0.783, 0.238, 0.769, 0.216, 1.008, 0.03, 0.985, 0.238, 0.769, 0.393, 0.821, 0.406, 0.996, 0.216, 1.008, 0.393, 0.821, 0.572, 0.799, 0.627, 1.036, 0.406, 0.996, 0.572, 0.799, 0.763, 0.813, 0.798, 1.013, 0.627, 1.036, 0.763, 0.813, 1.021, 0.806, 0.965, 1.016, 0.798, 1.013))
bpy.ops.object.mode_set(mode='EDIT')
#@blender IE = {'window': bpy.context.window_manager.windows[0], 'area': [a for a in bpy.context.window_manager.windows[0].screen.areas if a.type == 'DOPESHEET_EDITOR'][0]}
#@blender IE['area'].type = 'IMAGE_EDITOR'
#@blender IE['area'].spaces.active.mode = 'UV'
#@blender IE['region'] = [r for r in IE['area'].regions if r.type == 'WINDOW'][0]
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[7, 8, 13, 14])
#@frap bpy.ops.uv.snap_cursor(target='SELECTED')
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.snap_cursor(target='SELECTED') == {'FINISHED'}
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0])
#@frap bpy.ops.uv.snap_selected(target='CURSOR')
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.snap_selected(target='CURSOR') == {'FINISHED'}
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[20, 21, 26, 27])
#@frap bpy.ops.uv.snap_selected(target='CURSOR_OFFSET')
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.snap_selected(target='CURSOR_OFFSET') == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].uv_layers.new(name='cursor')
bpy.ops.object.mode_set(mode='EDIT')
#@frap bpy.ops.uv.snap_cursor()
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.snap_cursor() == {'FINISHED'}
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[33])
#@frap bpy.ops.uv.snap_selected(target='CURSOR')
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.snap_selected(target='CURSOR') == {'FINISHED'}
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[1, 2, 3])
#@frap bpy.ops.uv.snap_selected(target='PIXELS')
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.snap_selected(target='PIXELS') == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].uv_layers.new(name='pixels')
bpy.ops.object.mode_set(mode='EDIT')
#@frap bpy.ops.uv.snap_cursor(target='ORIGIN')
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.snap_cursor(target='ORIGIN') == {'FINISHED'}
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[35])
#@frap bpy.ops.uv.snap_selected(target='CURSOR')
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.snap_selected(target='CURSOR') == {'FINISHED'}
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[4, 5])
#@frap bpy.ops.uv.move_on_axis(type='PIXEL', axis='Y', distance=3)
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.move_on_axis(type='PIXEL', axis='Y', distance=3) == {'FINISHED'}
#@frap bpy.ops.uv.move_on_axis(type='DYNAMIC', distance=-2)
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.move_on_axis(type='DYNAMIC', distance=-2) == {'FINISHED'}
#@frap bpy.ops.uv.move_on_axis()
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.move_on_axis() == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].uv_layers.new(name='move')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[28, 29, 34])
#@frap bpy.ops.uv.snap_cursor(target='SELECTED')
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.snap_cursor(target='SELECTED') == {'FINISHED'}
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[9, 10, 15, 16, 21, 22])
#@frap bpy.ops.uv.arrange_islands(initial_position='CURSOR', axis='X', align='CENTER')
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.arrange_islands(initial_position='CURSOR', axis='X', align='CENTER') == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].uv_layers.new(name='arrange_cursor')
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.scene.tool_settings.use_uv_select_sync = False
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[6, 7, 11])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
#@frap bpy.ops.uv.snap_selected(target='ADJACENT_UNSELECTED')
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.snap_selected(target='ADJACENT_UNSELECTED') == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].uv_layers.new(name='adjacent')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[18])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
#@frap bpy.ops.uv.snap_cursor(target='SELECTED')
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.snap_cursor(target='SELECTED') == {'FINISHED'}
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[3])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
#@frap bpy.ops.uv.snap_selected(target='CURSOR_OFFSET')
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.snap_selected(target='CURSOR_OFFSET') == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
