# WP 8.3.8: uv.select_overlap selects UV faces overlapping another face: faces 0/1 overlap,
# 2/3 only share an edge (no overlap), 4/5 are identical (overlap); extend keeps the selection;
# with use_uv_select_island the islands of overlapping faces; with sync selection the mesh.
# golden: startup=factory compare=mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].uv_layers['UVMap'].data.foreach_set('uv', (0, 0, 1, 0, 1, 1, 0, 1, 0.5, 0.5, 1.5, 0.5, 1.5, 1.5, 0.5, 1.5, 3, 0, 4, 0, 4, 1, 3, 1, 4, 0, 5, 0, 5, 1, 4, 1, 7, 0, 8, 0, 8, 1, 7, 1, 7, 0, 8, 0, 8, 1, 7, 1))
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.scene.tool_settings.use_uv_select_sync = False
bpy.ops.uv.select_overlap()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'plain_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'plain_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'plain_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[2])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.select_overlap(extend=True)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'extend_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'extend_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'extend_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.scene.tool_settings.use_uv_select_island = True
bpy.ops.uv.select_overlap()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'island_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'island_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'island_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.scene.tool_settings.use_uv_select_island = False
bpy.context.scene.tool_settings.use_uv_select_sync = True
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.uv.select_overlap()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'sync_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'sync_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'sync_f'
bpy.data.meshes['Cube'].attributes['.select_vert'].name = 'sync_mv'
bpy.data.meshes['Cube'].attributes['.select_edge'].name = 'sync_me'
bpy.data.meshes['Cube'].attributes['.select_poly'].name = 'sync_mf'
