# WP 8.3.1: mesh.select_mirror on Suzanne (symmetric in X) and a grid (symmetric in X and Y):
# each axis alone, without and with extend, two axes in one call (each starting from the
# previous axis' result), all three axes, edges and faces, a vertex moved off its mirror (it
# fails: WARNING), and a hidden mirror. Intermediate selections are kept as renamed
# attributes.
# golden: startup=empty compare=mesh:Suzanne,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 2, 47, 100])
bpy.ops.mesh.select_mirror()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Suzanne'].attributes['.select_vert'].name = 'x'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 2, 47, 100])
bpy.ops.mesh.select_mirror(extend=True)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Suzanne'].attributes['.select_vert'].name = 'x_extend'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 2, 47, 100])
bpy.ops.mesh.select_mirror(axis={'Y'})
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[3, 40, 41, 200])
bpy.ops.mesh.select_mirror(axis={'X'})
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Suzanne'].attributes['.select_edge'].name = 'edges'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[5, 6, 300])
bpy.ops.mesh.select_mirror(axis={'X'}, extend=True)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Suzanne'].attributes['.select_poly'].name = 'faces'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='VERT')
bpy.ops.frap.select_elements(type='VERT', indices=[10])
bpy.ops.transform.translate(value=(0.01, 0, 0))
bpy.ops.frap.select_elements(type='VERT', indices=[10, 12])
bpy.ops.mesh.select_mirror(extend=True)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Suzanne'].attributes['.select_vert'].name = 'moved'
bpy.ops.object.select_all(action='DESELECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=5, y_subdivisions=5, location=(4, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 8])
bpy.ops.mesh.select_mirror(axis={'X', 'Y'})
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.select_vert'].name = 'xy'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 8])
bpy.ops.mesh.select_mirror(axis={'X', 'Y', 'Z'}, extend=True)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes['Grid'].attributes['.select_vert'].name = 'xyz'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[5])
bpy.ops.mesh.hide(unselected=False)
bpy.ops.frap.select_elements(type='VERT', indices=[0, 7])
bpy.ops.mesh.select_mirror(axis={'X'})
