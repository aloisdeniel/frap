# WP 2.10: the edit operators work on every mesh in Edit Mode (multi-object edit, each mesh
# once): poke, quads_convert_to_tris, split and separate on two cubes edited together; a
# mesh without a selection is skipped.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cube.001,mesh:Cube.002 tier=T1
import bpy
bpy.ops.mesh.primitive_cube_add(size=1, location=(3, 0, 0))
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.context.objects_in_mode_unique_data[1].name
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[5], object='Cube')
bpy.ops.frap.select_elements(type='FACE', indices=[2], object='Cube.001')
bpy.ops.mesh.poke()
bpy.ops.frap.select_elements(type='FACE', indices=[0], object='Cube.001')
bpy.ops.mesh.quads_convert_to_tris(quad_method='FIXED')
bpy.ops.mesh.split()
bpy.ops.mesh.separate()
bpy.data.objects['Cube.002'].data.name
