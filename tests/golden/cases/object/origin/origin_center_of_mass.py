# WP 1.21 (T1), object.origin_set(ORIGIN_CENTER_OF_MASS): area-weighted face centroids.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.data.meshes['Cube'].vertices[0].co = (3, 3, 3)
bpy.ops.object.origin_set(type='ORIGIN_CENTER_OF_MASS')
