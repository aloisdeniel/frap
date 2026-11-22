# WP 8.3.1: mesh.select_similar's edge types on a subdivided cube scaled (1, 2, 3):
# EDGE_LENGTH (world space, EQUAL/GREATER/LESS, a threshold), EDGE_DIR (either direction),
# EDGE_FACE_ANGLE, EDGE_CREASE and EDGE_BEVEL (with a layer, and without one: every weight is
# 0), EDGE_SEAM and EDGE_SHARP (one kind selected, and a mix, which selects every edge),
# EDGE_FREESTYLE without and with a `freestyle_edge` layer, EDGE_FACES after deleting faces,
# and the error without a selected edge. Intermediate selections are kept as renamed
# attributes.
# golden: startup=empty compare=mesh:Cube tier=T1
import bpy
bpy.ops.mesh.primitive_cube_add(scale=(1, 2, 3))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.subdivide(number_cuts=1)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes.new('crease_edge', 'FLOAT', 'EDGE')
bpy.data.meshes['Cube'].attributes['crease_edge'].data[2].value = 0.5
bpy.data.meshes['Cube'].attributes['crease_edge'].data[7].value = 0.5
bpy.data.meshes['Cube'].attributes['crease_edge'].data[11].value = 1.0
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[0])
bpy.ops.mesh.select_similar(type='EDGE_LENGTH')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.select_edge'].name = 'len_eq'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='EDGE', indices=[0])
bpy.ops.mesh.select_similar(type='EDGE_LENGTH', compare='GREATER', threshold=0.5)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.select_edge'].name = 'len_gt'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='EDGE', indices=[5])
bpy.ops.mesh.select_similar(type='EDGE_LENGTH', compare='LESS', threshold=0.0)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.select_edge'].name = 'len_lt'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='EDGE', indices=[3])
bpy.ops.mesh.select_similar(type='EDGE_DIR')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.select_edge'].name = 'dir'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='EDGE', indices=[1])
bpy.ops.mesh.select_similar(type='EDGE_FACE_ANGLE')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.select_edge'].name = 'face_angle'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='EDGE', indices=[2])
bpy.ops.mesh.select_similar(type='EDGE_CREASE')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.select_edge'].name = 'crease_eq'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='EDGE', indices=[2])
bpy.ops.mesh.select_similar(type='EDGE_CREASE', compare='GREATER', threshold=0.25)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.select_edge'].name = 'crease_gt'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='EDGE', indices=[4])
bpy.ops.mesh.select_similar(type='EDGE_BEVEL')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.select_edge'].name = 'bevel_none'
bpy.data.meshes['Cube'].attributes.new('bevel_weight_edge', 'FLOAT', 'EDGE')
bpy.data.meshes['Cube'].attributes['bevel_weight_edge'].data[4].value = 0.3
bpy.data.meshes['Cube'].attributes['bevel_weight_edge'].data[9].value = 0.31
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='EDGE', indices=[4])
bpy.ops.mesh.select_similar(type='EDGE_BEVEL', threshold=0.02)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.select_edge'].name = 'bevel_eq'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='EDGE', indices=[6, 8, 10])
bpy.ops.mesh.mark_seam()
bpy.ops.frap.select_elements(type='EDGE', indices=[12, 14])
bpy.ops.mesh.mark_sharp()
bpy.ops.frap.select_elements(type='EDGE', indices=[6])
bpy.ops.mesh.select_similar(type='EDGE_SEAM')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.select_edge'].name = 'seam'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='EDGE', indices=[13])
bpy.ops.mesh.select_similar(type='EDGE_SHARP')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.select_edge'].name = 'sharp_no'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='EDGE', indices=[12])
bpy.ops.mesh.select_similar(type='EDGE_SHARP')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.select_edge'].name = 'sharp_yes'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='EDGE', indices=[6, 7])
bpy.ops.mesh.select_similar(type='EDGE_SEAM')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.select_edge'].name = 'seam_mix'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='EDGE', indices=[7])
bpy.ops.mesh.select_similar(type='EDGE_FREESTYLE')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.select_edge'].name = 'freestyle_none'
bpy.data.meshes['Cube'].attributes.new('freestyle_edge', 'BOOLEAN', 'EDGE')
bpy.data.meshes['Cube'].attributes['freestyle_edge'].data[7].value = True
bpy.data.meshes['Cube'].attributes['freestyle_edge'].data[17].value = True
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='EDGE', indices=[7])
bpy.ops.mesh.select_similar(type='EDGE_FREESTYLE')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.select_edge'].name = 'freestyle'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1])
bpy.ops.mesh.delete(type='FACE')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_similar(type='EDGE_FACES')
bpy.ops.frap.select_elements(type='EDGE', indices=[0])
bpy.ops.mesh.select_similar(type='EDGE_FACES', compare='GREATER')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Cube'].attributes['.select_edge'].name = 'faces_gt'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 1, 2])
bpy.ops.mesh.select_similar(type='EDGE_FACES', compare='LESS')
