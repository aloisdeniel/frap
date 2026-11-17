# WP 1.21 (T1), object.shade_smooth removes sharp_face and keeps the sharp edges.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.data.meshes['Cube'].edges[0].use_edge_sharp = True
bpy.ops.object.shade_smooth()
