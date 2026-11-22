# WP 8.3.3: mesh.sort_elements on two meshes edited together: RANDOMIZE adds the hash of the
# second object's id.name to the seed, VIEW_ZAXIS uses each object's world matrix, and a mesh
# without a selection of the affected types is skipped.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cube.001 tier=T1
import bpy
bpy.ops.mesh.primitive_cube_add(size=1, location=(3, 0, 0), rotation=(0.3, 0.5, 0.2))
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.subdivide(number_cuts=1)
bpy.ops.mesh.sort_elements(type='RANDOMIZE', elements={'VERT', 'EDGE', 'FACE'}, seed=5)
bpy.ops.mesh.sort_elements(type='VIEW_ZAXIS', elements={'VERT', 'FACE'}, reverse=True)
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 2, 5, 9, 11, 20], object='Cube.001')
bpy.ops.mesh.sort_elements(type='REVERSE', elements={'VERT'})
bpy.ops.mesh.sort_elements(type='CURSOR_DISTANCE', elements={'FACE'})
