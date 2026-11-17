# WP 1.21 (T1), object.origin_set default type GEOMETRY_ORIGIN: the mesh moves so that its
# vertex median is at the origin; the object stays.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.data.meshes['Cube'].vertices[0].co = (3, 3, 3)
bpy.data.objects['Cube'].location = (1, 0, 0)
bpy.ops.object.origin_set()
