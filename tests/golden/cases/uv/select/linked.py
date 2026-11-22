# WP 8.3.8: uv.select_linked without sync selection selects the UV islands of the selected UVs
# (the factory cube is one island; with per-face UVs every face is its own island), with delimit
# SEAM / MATERIAL / SHARP on the joined layout.
# golden: startup=factory compare=mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.scene.tool_settings.use_uv_select_sync = False
bpy.ops.uv.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[4])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.select_linked()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'joined_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'joined_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'joined_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='EDGE', indices=[2, 4, 7, 11])
bpy.ops.mesh.mark_seam()
bpy.ops.mesh.mark_sharp()
bpy.ops.frap.select_elements(type='FACE', indices=[4])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.select_linked(delimit={'SEAM'})
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'seam_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'seam_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'seam_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[0])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.select_linked(delimit={'SHARP'})
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'sharp_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'sharp_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'sharp_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.mark_seam(clear=True)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].polygons.foreach_set('material_index', [0, 1, 0, 1, 0, 0])
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[2])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.select_linked(delimit={'MATERIAL'})
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'material_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'material_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'material_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].uv_layers['UVMap'].data.foreach_set('uv', (0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0, 2.0, 0.0, 3.0, 0.0, 3.0, 1.0, 2.0, 1.0, 4.0, 0.0, 5.0, 0.0, 5.0, 1.0, 4.0, 1.0, 6.0, 0.0, 7.0, 0.0, 7.0, 1.0, 6.0, 1.0, 8.0, 0.0, 9.0, 0.0, 9.0, 1.0, 8.0, 1.0, 10.0, 0.0, 11.0, 0.0, 11.0, 1.0, 10.0, 1.0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[1])
bpy.ops.uv.select_all(action='SELECT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.select_linked()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.uv_select_vert'].name = 'split_v'
bpy.data.meshes['Cube'].attributes['.uv_select_edge'].name = 'split_e'
bpy.data.meshes['Cube'].attributes['.uv_select_face'].name = 'split_f'
