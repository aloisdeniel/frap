# WP 4.6: mesh.intersect (Intersect (Knife), EXACT; T3). A plane added in Edit Mode cuts the
# default cube's four side faces (Merge: the cut line is shared); with the default CUT the
# plane is detached from the cube along the cut. The cube part stays closed.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T3 check=faces:11-40
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.primitive_plane_add(size=3, location=(0, 0, 0.25))
bpy.ops.mesh.intersect(separate_mode='NONE')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.primitive_plane_add(size=3, location=(0, 0, -0.5), rotation=(0.2, 0, 0))
bpy.ops.mesh.intersect()
bpy.ops.object.mode_set(mode='OBJECT')
