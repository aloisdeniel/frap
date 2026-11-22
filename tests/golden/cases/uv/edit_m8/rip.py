# WP 8.3.9: uv.rip / uv.rip_move on a jittered grid with sync selection: a single UV (the fan
# closest to location stays selected), a UV edge row (the side away from location is
# deselected), selected faces (isolated), the errors (nothing to rip at a corner, SHARED_VERTEX
# sticky selection), and rip_move translating what stays selected (headless-extended: Blender
# runs it in an Image Editor, where TRANSFORM_OT_translate moves UVs). The UV selection after
# each rip is kept in renamed .uv_select_* attributes.
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
bpy.ops.frap.select_elements(type='VERT', indices=[14])
bpy.ops.uv.rip(location=(0.45, 0.3))
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.uv_select_vert'].name = 'single_v'
bpy.data.meshes['Grid'].attributes['.uv_select_edge'].name = 'single_e'
bpy.data.meshes['Grid'].attributes['.uv_select_face'].name = 'single_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[19, 20, 21])
bpy.ops.uv.rip(location=(0.5, 0.75))
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.uv_select_vert'].name = 'edge_v'
bpy.data.meshes['Grid'].attributes['.uv_select_edge'].name = 'edge_e'
bpy.data.meshes['Grid'].attributes['.uv_select_face'].name = 'edge_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[7, 8, 13, 14])
bpy.ops.uv.rip()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.uv_select_vert'].name = 'faces_v'
bpy.data.meshes['Grid'].attributes['.uv_select_edge'].name = 'faces_e'
bpy.data.meshes['Grid'].attributes['.uv_select_face'].name = 'faces_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0])
bpy.ops.uv.rip(location=(0.5, 0.5))
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[22])
#@frap bpy.ops.uv.rip_move(UV_OT_rip={'location': (0.7, 0.55)}, TRANSFORM_OT_translate={'value': (0.1, -0.05, 0.0)})
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.rip_move(UV_OT_rip={'location': (0.7, 0.55)}, TRANSFORM_OT_translate={'value': (0.1, -0.05, 0.0)}) == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].uv_layers.new(name='rip_move_single')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[26, 27, 28])
#@frap bpy.ops.uv.rip_move(UV_OT_rip={'location': (0.6, 0.9)}, TRANSFORM_OT_translate={'value': (0.2, 0.3, 0.0), 'constraint_axis': (False, True, False)})
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.rip_move(UV_OT_rip={'location': (0.6, 0.9)}, TRANSFORM_OT_translate={'value': (0.2, 0.3, 0.0), 'constraint_axis': (False, True, False)}) == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].uv_layers.new(name='rip_move_edge')
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.scene.tool_settings.uv_sticky_select_mode = 'SHARED_VERTEX'
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[9])
bpy.ops.uv.rip()
bpy.ops.object.mode_set(mode='OBJECT')
