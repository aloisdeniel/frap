# WP 1.21 (T1), object.shade_flat re-creates sharp_face (all true) at the end of the attributes;
# keep_sharp_edges=False removes sharp_edge.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.shade_smooth()
bpy.data.meshes['Cube'].edges[0].use_edge_sharp = True
bpy.ops.object.shade_flat()
bpy.ops.object.shade_flat(keep_sharp_edges=False)
