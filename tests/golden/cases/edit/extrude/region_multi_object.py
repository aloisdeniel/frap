# WP 2.5: mesh.extrude_region in multi-object Edit Mode: each mesh extrudes its own selection.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cube.001 tier=T1 order=canonical
import bpy
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0), size=1)
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[1])
bpy.ops.frap.select_elements(type='FACE', indices=[2, 3], object='Cube')
bpy.ops.mesh.extrude_region()
