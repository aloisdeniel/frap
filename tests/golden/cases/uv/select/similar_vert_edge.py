# WP 8.3.8: uv.select_similar (headless-extended) in the vertex UV mode (PIN, and the default
# type, which is PIN), the edge mode (LENGTH, LENGTH_3D, PIN with EQUAL/GREATER/LESS and a
# threshold), on per-face UVs of two sizes and pinned vertices 1 and 3. LESS compares against
# FLT_MAX squared (infinite) in Blender's kd-tree and never selects.
# golden: startup=factory compare=mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].uv_layers['UVMap'].data.foreach_set('uv', (0, 0, 1, 0, 1, 1, 0, 1,  2, 0, 4, 0, 4, 2, 2, 2,  5, 0, 6, 0, 6, 1, 5, 1,  7, 0, 7, 1, 8, 1, 8, 0,  -1, 0, 0, 0, 0, 1, -1, 1,  10, 0, 11, 0, 11, 1, 10, 1))
bpy.data.objects['Cube'].scale = (1, 2, 3)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[1, 3])
bpy.ops.uv.pin()
bpy.ops.mesh.select_all(action='SELECT')
bpy.context.scene.tool_settings.use_uv_select_sync = False
#@blender IE = {'window': bpy.context.window_manager.windows[0], 'area': [a for a in bpy.context.window_manager.windows[0].screen.areas if a.type == 'DOPESHEET_EDITOR'][0]}
#@blender IE['area'].type = 'IMAGE_EDITOR'
#@blender IE['area'].spaces.active.mode = 'UV'
#@blender IE['region'] = [r for r in IE['area'].regions if r.type == 'WINDOW'][0]
bpy.ops.uv.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[0])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
#@frap bpy.ops.uv.select_similar(type='PIN')
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.select_similar(type='PIN') == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'pin_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'pin_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'pin_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[0])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
#@frap bpy.ops.uv.select_similar()
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.select_similar() == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'default_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'default_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'default_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.scene.tool_settings.uv_select_mode = 'EDGE'
bpy.ops.frap.select_elements(type='FACE', indices=[0])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.select_split()
#@frap bpy.ops.uv.select_similar(type='LENGTH')
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.select_similar(type='LENGTH') == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'length_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'length_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'length_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[1])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
#@frap bpy.ops.uv.select_similar(type='LENGTH_3D', compare='GREATER')
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.select_similar(type='LENGTH_3D', compare='GREATER') == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'len3d_gt_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'len3d_gt_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'len3d_gt_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[1])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
#@frap bpy.ops.uv.select_similar(type='LENGTH_3D', compare='LESS', threshold=0.5)
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.select_similar(type='LENGTH_3D', compare='LESS', threshold=0.5) == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'len3d_lt_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'len3d_lt_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'len3d_lt_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[2])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
#@frap bpy.ops.uv.select_similar(type='PIN', threshold=0.5)
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.select_similar(type='PIN', threshold=0.5) == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'epin_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'epin_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'epin_f'
