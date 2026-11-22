# WP 8.3.8: uv.shortest_path_select (headless-extended) on a 4x4 quad grid without sync
# selection: between two selected UV vertices (also with topology distance and a skip/nth
# pattern), two UV edges (face stepping), two faces, and use_fill regions; CANCELLED without two
# elements.
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
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 24])
bpy.context.scene.tool_settings.use_uv_select_sync = True
bpy.context.scene.tool_settings.use_uv_select_sync = False
bpy.ops.mesh.select_all(action='SELECT')
#@frap bpy.ops.uv.shortest_path_select()
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.shortest_path_select() == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.uv_select_vert'].name = 'vert_v'
bpy.data.meshes['Grid'].attributes['.uv_select_edge'].name = 'vert_e'
bpy.data.meshes['Grid'].attributes['.uv_select_face'].name = 'vert_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 18])
bpy.context.scene.tool_settings.use_uv_select_sync = True
bpy.context.scene.tool_settings.use_uv_select_sync = False
bpy.ops.mesh.select_all(action='SELECT')
#@frap bpy.ops.uv.shortest_path_select(use_topology_distance=True, skip=1, nth=1)
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.shortest_path_select(use_topology_distance=True, skip=1, nth=1) == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.uv_select_vert'].name = 'topo_v'
bpy.data.meshes['Grid'].attributes['.uv_select_edge'].name = 'topo_e'
bpy.data.meshes['Grid'].attributes['.uv_select_face'].name = 'topo_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 18])
bpy.context.scene.tool_settings.use_uv_select_sync = True
bpy.context.scene.tool_settings.use_uv_select_sync = False
bpy.ops.mesh.select_all(action='SELECT')
#@frap bpy.ops.uv.shortest_path_select(use_fill=True)
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.shortest_path_select(use_fill=True) == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.uv_select_vert'].name = 'vfill_v'
bpy.data.meshes['Grid'].attributes['.uv_select_edge'].name = 'vfill_e'
bpy.data.meshes['Grid'].attributes['.uv_select_face'].name = 'vfill_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.scene.tool_settings.uv_select_mode = 'FACE'
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 15])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
#@frap bpy.ops.uv.shortest_path_select()
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.shortest_path_select() == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.uv_select_vert'].name = 'face_v'
bpy.data.meshes['Grid'].attributes['.uv_select_edge'].name = 'face_e'
bpy.data.meshes['Grid'].attributes['.uv_select_face'].name = 'face_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 10])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
#@frap bpy.ops.uv.shortest_path_select(use_fill=True)
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.shortest_path_select(use_fill=True) == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.uv_select_vert'].name = 'ffill_v'
bpy.data.meshes['Grid'].attributes['.uv_select_edge'].name = 'ffill_e'
bpy.data.meshes['Grid'].attributes['.uv_select_face'].name = 'ffill_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 10])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
#@frap bpy.ops.uv.shortest_path_select(use_face_step=True)
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.shortest_path_select(use_face_step=True) == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.uv_select_vert'].name = 'fstep_v'
bpy.data.meshes['Grid'].attributes['.uv_select_edge'].name = 'fstep_e'
bpy.data.meshes['Grid'].attributes['.uv_select_face'].name = 'fstep_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.scene.tool_settings.uv_select_mode = 'EDGE'
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 30])
bpy.context.scene.tool_settings.use_uv_select_sync = True
bpy.context.scene.tool_settings.use_uv_select_sync = False
bpy.ops.mesh.select_all(action='SELECT')
#@frap bpy.ops.uv.shortest_path_select()
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.shortest_path_select() == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.uv_select_vert'].name = 'edge_v'
bpy.data.meshes['Grid'].attributes['.uv_select_edge'].name = 'edge_e'
bpy.data.meshes['Grid'].attributes['.uv_select_face'].name = 'edge_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 30])
bpy.context.scene.tool_settings.use_uv_select_sync = True
bpy.context.scene.tool_settings.use_uv_select_sync = False
bpy.ops.mesh.select_all(action='SELECT')
#@frap bpy.ops.uv.shortest_path_select(use_face_step=True)
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.shortest_path_select(use_face_step=True) == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.uv_select_vert'].name = 'estep_v'
bpy.data.meshes['Grid'].attributes['.uv_select_edge'].name = 'estep_e'
bpy.data.meshes['Grid'].attributes['.uv_select_face'].name = 'estep_f'
