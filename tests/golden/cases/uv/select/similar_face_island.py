# WP 8.3.8: uv.select_similar in the face UV mode (AREA, AREA_3D, MATERIAL, OBJECT, SIDES,
# WINDING) and with use_uv_select_island (AREA, AREA_3D, FACE: faces per island; faces 0 and 4
# form one island, the others are single faces of two sizes, face 3 clockwise; an island counts
# as selected when its first face is); threshold stored in the tool settings when given.
# golden: startup=factory compare=mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].uv_layers['UVMap'].data.foreach_set('uv', (0, 0, 1, 0, 1, 1, 0, 1,  2, 0, 4, 0, 4, 2, 2, 2,  5, 0, 6, 0, 6, 1, 5, 1,  7, 0, 7, 1, 8, 1, 8, 0,  -1, 0, 0, 0, 0, 1, -1, 1,  10, 0, 11, 0, 11, 1, 10, 1))
bpy.data.objects['Cube'].scale = (1, 2, 3)
bpy.data.meshes['Cube'].materials.append(bpy.data.materials['Material'])
bpy.data.materials.new('M2')
bpy.data.meshes['Cube'].materials.append(bpy.data.materials['M2'])
bpy.data.meshes['Cube'].polygons.foreach_set('material_index', [0, 1, 0, 1, 2, 2])
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.scene.tool_settings.use_uv_select_sync = False
#@blender IE = {'window': bpy.context.window_manager.windows[0], 'area': [a for a in bpy.context.window_manager.windows[0].screen.areas if a.type == 'DOPESHEET_EDITOR'][0]}
#@blender IE['area'].type = 'IMAGE_EDITOR'
#@blender IE['area'].spaces.active.mode = 'UV'
#@blender IE['region'] = [r for r in IE['area'].regions if r.type == 'WINDOW'][0]
bpy.context.scene.tool_settings.uv_select_mode = 'FACE'
bpy.ops.uv.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[0])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
#@frap bpy.ops.uv.select_similar(type='AREA')
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.select_similar(type='AREA') == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'area_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'area_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'area_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[0])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
#@frap bpy.ops.uv.select_similar(type='AREA_3D')
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.select_similar(type='AREA_3D') == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'area3d_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'area3d_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'area3d_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[1])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
#@frap bpy.ops.uv.select_similar(type='MATERIAL')
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.select_similar(type='MATERIAL') == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'material_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'material_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'material_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[1])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
#@frap bpy.ops.uv.select_similar(type='OBJECT')
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.select_similar(type='OBJECT') == {'FINISHED'}
#@frap bpy.ops.uv.select_similar(type='SIDES')
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.select_similar(type='SIDES') == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'obj_sides_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'obj_sides_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'obj_sides_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[3])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
#@frap bpy.ops.uv.select_similar(type='WINDING')
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.select_similar(type='WINDING') == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'winding_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'winding_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'winding_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[0])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
#@frap bpy.ops.uv.select_similar(type='AREA', compare='GREATER', threshold=0.25)
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.select_similar(type='AREA', compare='GREATER', threshold=0.25) == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'area_gt_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'area_gt_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'area_gt_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.scene.tool_settings.use_uv_select_island = True
bpy.ops.frap.select_elements(type='FACE', indices=[1])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
#@frap bpy.ops.uv.select_similar(type='FACE')
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.select_similar(type='FACE') == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'island_face_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'island_face_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'island_face_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[2])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
#@frap bpy.ops.uv.select_similar(type='AREA')
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.select_similar(type='AREA') == {'FINISHED'}
#@frap bpy.ops.uv.select_similar(type='AREA_3D', compare='GREATER')
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.select_similar(type='AREA_3D', compare='GREATER') == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'island_area_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'island_area_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'island_area_f'
