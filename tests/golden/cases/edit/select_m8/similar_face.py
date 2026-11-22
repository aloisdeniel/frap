# WP 8.3.1: mesh.select_similar's face types on a rotated, scaled Suzanne: FACE_SIDES
# (EQUAL/GREATER/LESS), FACE_AREA and FACE_PERIMETER (object 3x3 matrix, thresholds, LESS
# never matching), FACE_NORMAL and FACE_COPLANAR (world space), FACE_SMOOTH (one kind and a
# mix), FACE_FREESTYLE without and with a `freestyle_face` layer, FACE_MATERIAL (no slots,
# then two materials), the default type in face mode (VERT_NORMAL: it selects vertices) and a
# vertex type in face mode (Blender's TypeError). Intermediate selections are kept as renamed
# attributes.
# golden: startup=empty compare=mesh:Suzanne tier=T1
import bpy
bpy.ops.mesh.primitive_monkey_add(rotation=(0.3, 0, 0.2), scale=(1, 1, 2))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[0])
bpy.ops.mesh.select_similar(type='FACE_MATERIAL')
bpy.ops.mesh.select_similar(type='FACE_SIDES')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Suzanne'].attributes['.select_poly'].name = 'sides_eq'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1])
bpy.ops.mesh.select_similar(type='FACE_SIDES', compare='LESS')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Suzanne'].attributes['.select_poly'].name = 'sides_lt'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[40])
bpy.ops.mesh.select_similar(type='FACE_AREA', threshold=0.002)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Suzanne'].attributes['.select_poly'].name = 'area_eq'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[40])
bpy.ops.mesh.select_similar(type='FACE_AREA', compare='GREATER', threshold=0.0005)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Suzanne'].attributes['.select_poly'].name = 'area_gt'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[40])
bpy.ops.mesh.select_similar(type='FACE_AREA', compare='LESS')
bpy.ops.frap.select_elements(type='FACE', indices=[120])
bpy.ops.mesh.select_similar(type='FACE_PERIMETER', threshold=0.01)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Suzanne'].attributes['.select_poly'].name = 'perimeter'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[200])
bpy.ops.mesh.select_similar(type='FACE_NORMAL', threshold=0.05)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Suzanne'].attributes['.select_poly'].name = 'normal'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[200, 3])
bpy.ops.mesh.select_similar(type='FACE_COPLANAR', threshold=0.2)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Suzanne'].attributes['.select_poly'].name = 'coplanar'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[10, 11, 12, 300])
bpy.ops.mesh.faces_shade_smooth()
bpy.ops.frap.select_elements(type='FACE', indices=[11])
bpy.ops.mesh.select_similar(type='FACE_SMOOTH')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Suzanne'].attributes['.select_poly'].name = 'smooth'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[11, 13])
bpy.ops.mesh.select_similar(type='FACE_SMOOTH')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Suzanne'].attributes['.select_poly'].name = 'smooth_mix'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[5])
bpy.ops.mesh.select_similar(type='FACE_FREESTYLE')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Suzanne'].attributes['.select_poly'].name = 'freestyle_none'
bpy.data.meshes['Suzanne'].attributes.new('freestyle_face', 'BOOLEAN', 'FACE')
bpy.data.meshes['Suzanne'].attributes['freestyle_face'].data[5].value = True
bpy.data.meshes['Suzanne'].attributes['freestyle_face'].data[50].value = True
bpy.data.meshes['Suzanne'].materials.append(bpy.data.materials.new('A'))
bpy.data.meshes['Suzanne'].materials.append(bpy.data.materials.new('B'))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[5])
bpy.ops.mesh.select_similar(type='FACE_FREESTYLE')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Suzanne'].attributes['.select_poly'].name = 'freestyle'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[60, 61, 62, 63])
bpy.context.object.active_material_index = 1
bpy.ops.object.material_slot_assign()
bpy.ops.frap.select_elements(type='FACE', indices=[61])
bpy.ops.mesh.select_similar(type='FACE_MATERIAL')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Suzanne'].attributes['.select_poly'].name = 'material'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[7])
bpy.ops.mesh.select_similar(type='VERT_NORMAL')
bpy.ops.mesh.select_similar()
