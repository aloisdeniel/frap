# WP 2.10: mesh.separate(type='LOOSE') in Edit Mode: the cube plus a merged cube and plane
# make three parts; the edit object keeps the first (from vertex 0), the others become
# Cube.001 and Cube.002 with their elements in walk order. Then in Object Mode, LOOSE on a
# grid merged with a circle (selected editable objects only).
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cube.001,mesh:Cube.002,mesh:Grid,mesh:Grid.001 tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.primitive_cube_add(size=1, location=(3, 0, 0))
bpy.ops.mesh.primitive_plane_add(size=1, location=(0, 3, 0))
bpy.ops.mesh.separate(type='LOOSE')
bpy.data.meshes['Cube'].total_vert_sel
bpy.ops.mesh.separate(type='LOOSE')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=2, y_subdivisions=2, size=1, location=(0, -3, 0), enter_editmode=True)
bpy.ops.mesh.primitive_circle_add(vertices=5, radius=0.5, location=(0, -5, 0))
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Grid'].select_set(True)
bpy.ops.mesh.separate(type='LOOSE')
bpy.ops.mesh.separate(type='SELECTED')
bpy.ops.mesh.separate(type='MATERIAL')
