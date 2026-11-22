# WP 8.3.1: mesh.select_similar_region (BM_mesh_region_match): a 2x2 corner region of a 6x6
# grid, a region of a quad and a triangle on a grid with triangulated faces, two source
# regions at once, a strip of quads where nothing matches (WARNING, FINISHED), and fewer than
# two selected faces (Blender's error). Intermediate selections are kept as renamed
# attributes.
# golden: startup=empty compare=mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=6)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1, 6, 7])
bpy.ops.mesh.select_similar_region()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.select_poly'].name = 'corner'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[14, 15, 20, 21])
bpy.ops.mesh.select_similar_region()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.select_poly'].name = 'center'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1, 34, 35])
bpy.ops.mesh.select_similar_region()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.select_poly'].name = 'two_groups'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[8, 9, 10, 16, 22, 28])
bpy.ops.mesh.quads_convert_to_tris()
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[2, 30, 31])
bpy.ops.mesh.select_similar_region()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.select_poly'].name = 'mixed'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[3])
bpy.ops.mesh.select_similar_region()
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.select_similar_region()
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[4, 5, 11])
bpy.ops.mesh.select_similar_region()
