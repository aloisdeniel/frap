# WP 8.3.1: mesh.select_similar's vertex types on a scaled ico sphere: VERT_EDGES and
# VERT_FACES with EQUAL/GREATER/LESS, VERT_NORMAL with a threshold (stored in the tool
# settings and reused by the next call without one), the default type (VERT_NORMAL),
# VERT_CREASE (EQUAL/GREATER, and LESS, which Blender's kd-tree never satisfies), VERT_GROUPS
# (shared groups by name, and the INFO without any group), and the errors: nothing selected,
# a type outside the vertex list. Intermediate selections are kept as renamed attributes.
# golden: startup=empty compare=mesh:Icosphere tier=T1
import bpy
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=2, scale=(1, 1, 1.5))
bpy.data.meshes['Icosphere'].attributes.new('crease_vert', 'FLOAT', 'POINT')
bpy.data.meshes['Icosphere'].attributes['crease_vert'].data[3].value = 0.5
bpy.data.meshes['Icosphere'].attributes['crease_vert'].data[4].value = 0.5
bpy.data.meshes['Icosphere'].attributes['crease_vert'].data[9].value = 0.75
bpy.data.meshes['Icosphere'].attributes['crease_vert'].data[20].value = 0.25
bpy.data.objects['Icosphere'].vertex_groups.new(name='A')
bpy.data.objects['Icosphere'].vertex_groups.new(name='B')
bpy.data.objects['Icosphere'].vertex_groups['A'].add([1, 2, 3], 1.0, 'REPLACE')
bpy.data.objects['Icosphere'].vertex_groups['A'].add([4], 0.0, 'REPLACE')
bpy.data.objects['Icosphere'].vertex_groups['B'].add([10, 11, 12, 30], 0.5, 'REPLACE')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_similar(type='VERT_EDGES')
bpy.ops.frap.select_elements(type='VERT', indices=[5])
bpy.ops.mesh.select_similar(type='VERT_EDGES')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Icosphere'].attributes['.select_vert'].name = 'edges_eq'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[5])
bpy.ops.mesh.select_similar(type='VERT_EDGES', compare='GREATER')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Icosphere'].attributes['.select_vert'].name = 'edges_gt'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[5])
bpy.ops.mesh.select_similar(type='VERT_FACES', compare='LESS')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Icosphere'].attributes['.select_vert'].name = 'faces_lt'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[5, 33])
bpy.ops.mesh.select_similar(type='VERT_FACES', compare='EQUAL')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Icosphere'].attributes['.select_vert'].name = 'faces_eq'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[6])
bpy.ops.mesh.select_similar(type='VERT_NORMAL', threshold=0.3)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Icosphere'].attributes['.select_vert'].name = 'normal_03'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[14])
bpy.ops.mesh.select_similar()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Icosphere'].attributes['.select_vert'].name = 'normal_stored'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[14])
bpy.ops.mesh.select_similar(type='VERT_NORMAL', threshold=0.0)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Icosphere'].attributes['.select_vert'].name = 'normal_0'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[3])
bpy.ops.mesh.select_similar(type='VERT_CREASE')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Icosphere'].attributes['.select_vert'].name = 'crease_eq'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[20])
bpy.ops.mesh.select_similar(type='VERT_CREASE', compare='GREATER')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Icosphere'].attributes['.select_vert'].name = 'crease_gt'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[9])
bpy.ops.mesh.select_similar(type='VERT_CREASE', compare='LESS', threshold=0.1)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Icosphere'].attributes['.select_vert'].name = 'crease_lt'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[2, 30])
bpy.ops.mesh.select_similar(type='VERT_GROUPS')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Icosphere'].attributes['.select_vert'].name = 'groups'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[4])
bpy.ops.mesh.select_similar(type='VERT_GROUPS')
bpy.ops.mesh.select_similar(type='EDGE_LENGTH')
bpy.ops.frap.select_elements(type='VERT', indices=[25, 26])
bpy.ops.mesh.select_similar(type='VERT_EDGES', compare='LESS')
