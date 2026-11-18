# WP 2.11: two objects in Edit Mode, only one with a selection: only that mesh is
# beveled.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cube.001 tier=T1
import bpy
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0))
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[2], object='Cube')
bpy.ops.mesh.bevel(offset=0.3, affect='VERTICES')
