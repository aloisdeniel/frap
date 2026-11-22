# WP 8.3.1: mesh.select_ungrouped (poll failures without vertex groups, without weights and
# outside vertex select mode; replace and extend; a zero weight still counts as grouped) and
# mesh.select_by_pole_count on an ico sphere (valences 5 and 6) (every comparison, extend, exclude_nonmanifold on a
# mesh with boundaries, edge and face select modes). Intermediate selections are kept as
# renamed attributes.
# golden: startup=empty compare=mesh:Icosphere tier=T1
import bpy
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=2)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_ungrouped()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Icosphere'].vertex_groups.new(name='G')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_ungrouped()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Icosphere'].vertex_groups['G'].add([0, 1, 2, 3, 4, 5, 6, 7], 1.0, 'REPLACE')
bpy.data.objects['Icosphere'].vertex_groups['G'].add([8, 9], 0.0, 'REPLACE')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 20])
bpy.ops.mesh.select_ungrouped()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Icosphere'].attributes['.select_vert'].name = 'ungrouped'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 20])
bpy.ops.mesh.select_ungrouped(extend=True)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Icosphere'].attributes['.select_vert'].name = 'ungrouped_extend'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.mesh.select_ungrouped()
bpy.ops.mesh.select_mode(type='VERT')
bpy.ops.mesh.select_by_pole_count()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Icosphere'].attributes['.select_vert'].name = 'pole_ne4'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_by_pole_count(pole_count=5, type='EQUAL')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Icosphere'].attributes['.select_vert'].name = 'pole_eq5'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[12])
bpy.ops.mesh.select_by_pole_count(pole_count=6, type='LESS', extend=True)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Icosphere'].attributes['.select_vert'].name = 'pole_lt6'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.mesh.select_by_pole_count(pole_count=5, type='GREATER')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Icosphere'].attributes['.select_poly'].name = 'pole_gt5_faces'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1, 2, 3])
bpy.ops.mesh.delete(type='FACE')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.mesh.select_by_pole_count(pole_count=5, type='EQUAL')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Icosphere'].attributes['.select_edge'].name = 'pole_eq5_edges'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='VERT')
bpy.ops.mesh.select_by_pole_count(pole_count=5, type='EQUAL', exclude_nonmanifold=False)
