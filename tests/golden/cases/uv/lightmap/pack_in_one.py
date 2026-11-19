# WP 3.8: uv.lightmap_pack over two selected meshes packed together (PREF_PACK_IN_ONE, the
# default) in Object Mode. Blender iterates a Python set of meshes (order from memory
# addresses), frap the selected objects in order, so the layouts differ: T3 invariants.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Plane tier=T3 check=uv01,uv_no_overlap
import bpy
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.data.objects['Cube'].select_set(True)
bpy.ops.uv.lightmap_pack(PREF_CONTEXT='ALL_FACES')
