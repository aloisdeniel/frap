# WP 8.3.8: uv.select_all with UV sync selection (the factory setting): SELECT/DESELECT/TOGGLE
# select the whole mesh, INVERT inverts the mesh selection of the mesh select mode (vertex, then
# face mode). The mesh selection is dumped through its renamed attributes.
# golden: startup=factory compare=mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.uv.select_all(action='TOGGLE')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 1, 2])
bpy.ops.uv.select_all(action='INVERT')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'vinv_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'vinv_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'vinv_f'
bpy.data.meshes['Cube'].attributes['.select_vert'].name = 'vinv_mv'
bpy.data.meshes['Cube'].attributes['.select_edge'].name = 'vinv_me'
bpy.data.meshes['Cube'].attributes['.select_poly'].name = 'vinv_mf'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 5])
bpy.ops.uv.select_all(action='INVERT')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'finv_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'finv_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'finv_f'
bpy.data.meshes['Cube'].attributes['.select_vert'].name = 'finv_mv'
bpy.data.meshes['Cube'].attributes['.select_edge'].name = 'finv_me'
bpy.data.meshes['Cube'].attributes['.select_poly'].name = 'finv_mf'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.uv.select_all(action='TOGGLE')
bpy.ops.uv.select_all(action='DESELECT')
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'all_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'all_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'all_f'
bpy.data.meshes['Cube'].attributes['.select_vert'].name = 'all_mv'
bpy.data.meshes['Cube'].attributes['.select_edge'].name = 'all_me'
bpy.data.meshes['Cube'].attributes['.select_poly'].name = 'all_mf'
