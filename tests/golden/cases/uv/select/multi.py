# WP 8.3.8: the UV selection operators on two meshes in Edit Mode: TOGGLE decides across both
# (deselect when either has a selected UV), select_overlap finds faces overlapping a face of the
# other mesh (the cubes share their UV layout; two faces of the first are moved in UV space),
# select_similar OBJECT compares the object index (active first).
# golden: startup=factory compare=mesh:Cube,mesh:Cube.001 tier=T1
import bpy
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0))
bpy.data.objects['Cube'].select_set(True)
bpy.data.meshes['Cube'].uv_layers['UVMap'].data.foreach_set('uv', (1.125, 0.5, 1.375, 0.5, 1.375, 0.75, 1.125, 0.75, 0.875, 0.75, 1.125, 0.75, 1.125, 1.0, 0.875, 1.0, 0.375, 0.0, 0.625, 0.0, 0.625, 0.25, 0.375, 0.25, 0.125, 0.5, 0.375, 0.5, 0.375, 0.75, 0.125, 0.75, 0.375, 0.5, 0.625, 0.5, 0.625, 0.75, 0.375, 0.75, 0.375, 0.25, 0.625, 0.25, 0.625, 0.5, 0.375, 0.5))
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.scene.tool_settings.use_uv_select_sync = False
#@blender IE = {'window': bpy.context.window_manager.windows[0], 'area': [a for a in bpy.context.window_manager.windows[0].screen.areas if a.type == 'DOPESHEET_EDITOR'][0]}
#@blender IE['area'].type = 'IMAGE_EDITOR'
#@blender IE['area'].spaces.active.mode = 'UV'
#@blender IE['region'] = [r for r in IE['area'].regions if r.type == 'WINDOW'][0]
bpy.ops.uv.select_all(action='DESELECT')
bpy.ops.frap.select_elements(object='Cube', type='FACE', indices=[4])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.select_all(action='TOGGLE')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'toggle_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'toggle_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'toggle_f'
bpy.data.meshes['Cube.001'].attributes['.uv_select_vert'].name = 'toggle_v'
bpy.data.meshes['Cube.001'].attributes['.uv_select_edge'].name = 'toggle_e'
bpy.data.meshes['Cube.001'].attributes['.uv_select_face'].name = 'toggle_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.uv.select_overlap()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'overlap_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'overlap_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'overlap_f'
bpy.data.meshes['Cube.001'].attributes['.uv_select_vert'].name = 'overlap_v'
bpy.data.meshes['Cube.001'].attributes['.uv_select_edge'].name = 'overlap_e'
bpy.data.meshes['Cube.001'].attributes['.uv_select_face'].name = 'overlap_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.scene.tool_settings.uv_select_mode = 'FACE'
bpy.ops.frap.select_elements(object='Cube.001', type='FACE', indices=[2])
bpy.ops.frap.select_elements(object='Cube', type='FACE', indices=[])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
#@frap bpy.ops.uv.select_similar(type='OBJECT')
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.select_similar(type='OBJECT') == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'object_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'object_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'object_f'
bpy.data.meshes['Cube.001'].attributes['.uv_select_vert'].name = 'object_v'
bpy.data.meshes['Cube.001'].attributes['.uv_select_edge'].name = 'object_e'
bpy.data.meshes['Cube.001'].attributes['.uv_select_face'].name = 'object_f'
