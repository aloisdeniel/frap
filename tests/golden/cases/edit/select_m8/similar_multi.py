# WP 8.3.1: mesh.select_similar across two meshes in Edit Mode: the reference values of both
# meshes are gathered in world space, so FACE_AREA, FACE_NORMAL, FACE_MATERIAL (the same
# material on both meshes), EDGE_DIR and EDGE_LENGTH match elements of the other mesh (a
# rotated, flattened cube). Intermediate selections are kept as renamed attributes.
# golden: startup=empty compare=mesh:Cube,mesh:Cube.001 tier=T1
import bpy
bpy.ops.mesh.primitive_cube_add()
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('Shared'))
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('Other'))
bpy.ops.mesh.primitive_cube_add(location=(4, 0, 0), rotation=(0, 0, 0.7853981633974483), scale=(1, 1, 0.5))
bpy.data.meshes['Cube.001'].materials.append(bpy.data.materials['Other'])
bpy.data.meshes['Cube.001'].materials.append(bpy.data.materials['Shared'])
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[], object='Cube')
bpy.ops.frap.select_elements(type='FACE', indices=[1], object='Cube.001')
bpy.context.view_layer.objects.active = bpy.data.objects['Cube.001']
bpy.context.object.active_material_index = 1
bpy.ops.object.material_slot_assign()
bpy.ops.frap.select_elements(type='FACE', indices=[0], object='Cube')
bpy.ops.frap.select_elements(type='FACE', indices=[0], object='Cube.001')
bpy.ops.mesh.select_similar(type='FACE_AREA', threshold=0.01)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.select_poly'].name = 'area_f'
bpy.data.meshes['Cube.001'].attributes['.select_poly'].name = 'area_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[2], object='Cube')
bpy.ops.frap.select_elements(type='FACE', indices=[5], object='Cube.001')
bpy.ops.mesh.select_similar(type='FACE_NORMAL', threshold=0.01)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.select_poly'].name = 'normal_f'
bpy.data.meshes['Cube.001'].attributes['.select_poly'].name = 'normal_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[4], object='Cube')
bpy.ops.frap.select_elements(type='FACE', indices=[1], object='Cube.001')
bpy.ops.mesh.select_similar(type='FACE_MATERIAL')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.select_poly'].name = 'material_f'
bpy.data.meshes['Cube.001'].attributes['.select_poly'].name = 'material_f'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[0], object='Cube')
bpy.ops.frap.select_elements(type='EDGE', indices=[0], object='Cube.001')
bpy.ops.mesh.select_similar(type='EDGE_DIR', threshold=0.3)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.select_edge'].name = 'dir_e'
bpy.data.meshes['Cube.001'].attributes['.select_edge'].name = 'dir_e'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='EDGE', indices=[3], object='Cube')
bpy.ops.frap.select_elements(type='EDGE', indices=[], object='Cube.001')
bpy.ops.mesh.select_similar(type='EDGE_LENGTH', compare='GREATER', threshold=0.1)
