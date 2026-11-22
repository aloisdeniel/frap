# WP 8.3.8: uv.select_similar with sync selection: the mesh selection decides the selected
# elements (vertex mode: PIN; face mode: SIDES on a cube with an extra triangle-free layout),
# the result reaches the mesh selection.
# golden: startup=factory compare=mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
#@blender IE = {'window': bpy.context.window_manager.windows[0], 'area': [a for a in bpy.context.window_manager.windows[0].screen.areas if a.type == 'DOPESHEET_EDITOR'][0]}
#@blender IE['area'].type = 'IMAGE_EDITOR'
#@blender IE['area'].spaces.active.mode = 'UV'
#@blender IE['region'] = [r for r in IE['area'].regions if r.type == 'WINDOW'][0]
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[2, 6])
bpy.ops.uv.pin()
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[2])
#@frap bpy.ops.uv.select_similar(type='PIN')
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.select_similar(type='PIN') == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'pin_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'pin_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'pin_f'
bpy.data.meshes['Cube'].attributes['.select_vert'].name = 'pin_mv'
bpy.data.meshes['Cube'].attributes['.select_edge'].name = 'pin_me'
bpy.data.meshes['Cube'].attributes['.select_poly'].name = 'pin_mf'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[1])
#@frap bpy.ops.uv.select_similar(type='AREA', threshold=0.0)
#@blender with bpy.context.temp_override(**IE): assert bpy.ops.uv.select_similar(type='AREA', threshold=0.0) == {'FINISHED'}
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'area_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'area_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'area_f'
bpy.data.meshes['Cube'].attributes['.select_vert'].name = 'area_mv'
bpy.data.meshes['Cube'].attributes['.select_edge'].name = 'area_me'
bpy.data.meshes['Cube'].attributes['.select_poly'].name = 'area_mf'
