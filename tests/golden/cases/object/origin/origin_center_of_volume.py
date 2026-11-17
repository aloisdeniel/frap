# WP 1.21 (T1), object.origin_set(ORIGIN_CENTER_OF_VOLUME) on a closed and on an open mesh.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Plane tier=T1
import bpy
bpy.data.meshes['Cube'].vertices[0].co = (3, 3, 3)
bpy.ops.object.origin_set(type='ORIGIN_CENTER_OF_VOLUME')
bpy.ops.mesh.primitive_plane_add(location=(0, 0, 0))
bpy.data.meshes['Plane'].vertices[0].co = (-1, -1, 1)
bpy.ops.object.origin_set(type='ORIGIN_CENTER_OF_VOLUME')
