# WP 2.7: mesh.delete runs on every mesh in Edit Mode; a mesh without a matching selection is
# skipped (the operator still finishes when another one changed).
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cube.001 tier=T1
import bpy
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0), size=1)
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[2, 4])
bpy.ops.frap.select_elements(type='VERT', indices=[7], object='Cube')
bpy.ops.mesh.delete(type='VERT')
bpy.ops.frap.select_elements(type='VERT', indices=[0])
bpy.ops.mesh.delete(type='EDGE')
