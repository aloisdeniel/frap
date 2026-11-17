# WP 1.19 (T0): deleting the factory Cube leaves its mesh as an orphan, so the next cube is
# object `Cube` with mesh `Cube.001` (object and data names are uniquified independently). The
# rename goes through the new object's data: `mesh:Cube` (the orphan) must still exist.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Renamed tier=T0
import bpy
bpy.ops.object.delete()
bpy.ops.mesh.primitive_cube_add()
bpy.data.meshes['Cube.001'].name
bpy.data.objects['Cube'].data.name = 'Renamed'
bpy.data.meshes['Cube.001']
