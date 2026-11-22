# WP 8.3.9: uv.rip / uv.rip_move without sync selection (every face visible; the UV
# selection comes from turning sync selection off after selecting mesh vertices): a single UV,
# a UV edge row, rip_move, and selected faces (no other corner shares their selection without
# sync: CANCELLED).
# golden: startup=empty compare=mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=5, y_subdivisions=5, size=2)
bpy.data.meshes['Grid'].uv_layers['UVMap'].data.foreach_set('uv', (-0.014, -0.028, 0.212, -0.034, 0.17, 0.178, -0.006, 0.226, 0.212, -0.034, 0.403, -0.011, 0.41, 0.236, 0.17, 0.178, 0.403, -0.011, 0.565, 0.001, 0.606, 0.192, 0.41, 0.236, 0.565, 0.001, 0.763, -0.005, 0.838, 0.164, 0.606, 0.192, 0.763, -0.005, 0.966, -0.033, 1.029, 0.183, 0.838, 0.164, -0.006, 0.226, 0.17, 0.178, 0.185, 0.425, -0.028, 0.369, 0.17, 0.178, 0.41, 0.236, 0.374, 0.407, 0.185, 0.425, 0.41, 0.236, 0.606, 0.192, 0.611, 0.39, 0.374, 0.407, 0.606, 0.192, 0.838, 0.164, 0.804, 0.365, 0.611, 0.39, 0.838, 0.164, 1.029, 0.183, 0.965, 0.376, 0.804, 0.365, -0.028, 0.369, 0.185, 0.425, 0.185, 0.607, 0.014, 0.594, 0.185, 0.425, 0.374, 0.407, 0.396, 0.584, 0.185, 0.607, 0.374, 0.407, 0.611, 0.39, 0.624, 0.616, 0.396, 0.584, 0.611, 0.39, 0.804, 0.365, 0.78, 0.606, 0.624, 0.616, 0.804, 0.365, 0.965, 0.376, 1.002, 0.63, 0.78, 0.606, 0.014, 0.594, 0.185, 0.607, 0.238, 0.769, 0.018, 0.783, 0.185, 0.607, 0.396, 0.584, 0.393, 0.821, 0.238, 0.769, 0.396, 0.584, 0.624, 0.616, 0.572, 0.799, 0.393, 0.821, 0.624, 0.616, 0.78, 0.606, 0.763, 0.813, 0.572, 0.799, 0.78, 0.606, 1.002, 0.63, 1.021, 0.806, 0.763, 0.813, 0.018, 0.783, 0.238, 0.769, 0.216, 1.008, 0.03, 0.985, 0.238, 0.769, 0.393, 0.821, 0.406, 0.996, 0.216, 1.008, 0.393, 0.821, 0.572, 0.799, 0.627, 1.036, 0.406, 0.996, 0.572, 0.799, 0.763, 0.813, 0.798, 1.013, 0.627, 1.036, 0.763, 0.813, 1.021, 0.806, 0.965, 1.016, 0.798, 1.013))
bpy.ops.object.mode_set(mode='EDIT')
#@blender IE = {'window': bpy.context.window_manager.windows[0], 'area': [a for a in bpy.context.window_manager.windows[0].screen.areas if a.type == 'DOPESHEET_EDITOR'][0]}
#@blender IE['area'].type = 'IMAGE_EDITOR'
#@blender IE['area'].spaces.active.mode = 'UV'
#@blender IE['region'] = [r for r in IE['area'].regions if r.type == 'WINDOW'][0]
bpy.context.scene.tool_settings.use_uv_select_sync = True
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[14])
bpy.context.scene.tool_settings.use_uv_select_sync = False
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.rip(location=(0.45, 0.3))
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.uv_select_vert'].name = 'single_v'
bpy.data.meshes['Grid'].attributes['.uv_select_edge'].name = 'single_e'
bpy.data.meshes['Grid'].attributes['.uv_select_face'].name = 'single_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.scene.tool_settings.use_uv_select_sync = True
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[19, 20, 21])
bpy.context.scene.tool_settings.use_uv_select_sync = False
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.rip(location=(0.5, 0.75))
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.uv_select_vert'].name = 'edge_v'
bpy.data.meshes['Grid'].attributes['.uv_select_edge'].name = 'edge_e'
bpy.data.meshes['Grid'].attributes['.uv_select_face'].name = 'edge_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.scene.tool_settings.use_uv_select_sync = True
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[22])
bpy.context.scene.tool_settings.use_uv_select_sync = False
bpy.ops.mesh.select_all(action='SELECT')
#@frap bpy.ops.uv.rip_move(UV_OT_rip={'location': (0.0, 1.0)}, TRANSFORM_OT_translate={'value': (-0.05, 0.1, 0.0)})
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.rip_move(UV_OT_rip={'location': (0.0, 1.0)}, TRANSFORM_OT_translate={'value': (-0.05, 0.1, 0.0)}) == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].uv_layers.new(name='rip_move')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[6, 7])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.rip(location=(0.1, 0.1))
bpy.ops.object.mode_set(mode='OBJECT')
