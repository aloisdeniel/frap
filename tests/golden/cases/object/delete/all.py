# WP 1.19 (T0): select everything and delete it (the M1 exit transcript's first two lines),
# then delete again with nothing selected (CANCELLED), then add a primitive into the emptied
# scene: the orphan meshes keep their names.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Plane tier=T0
import bpy
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete()
bpy.ops.object.delete()
bpy.ops.mesh.primitive_plane_add()
bpy.ops.object.delete(use_global=True)
