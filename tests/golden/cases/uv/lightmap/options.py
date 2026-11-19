# WP 3.8: uv.lightmap_pack over two selected objects packed separately (PREF_PACK_IN_ONE=False):
# an n-gon circle (fitted into its bounding box) and a triangulated Suzanne without a UV map
# (one is added; triangles paired by shape); PREF_NEW_UVLAYER adds a copy of the active map,
# which stays inactive (the active one is written).
# golden: startup=empty compare=objects,context,mesh:Circle,mesh:Suzanne tier=T1 tol=1e-5
import bpy
bpy.ops.mesh.primitive_circle_add(vertices=7, fill_type='NGON', location=(0, 0, 0), rotation=(0.4, 0.2, 0))
bpy.ops.mesh.primitive_monkey_add(location=(3, 0, 0))
bpy.ops.mesh.uv_texture_remove()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.quads_convert_to_tris(quad_method='FIXED')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Circle'].select_set(True)
bpy.ops.uv.lightmap_pack(PREF_PACK_IN_ONE=False, PREF_BOX_DIV=3, PREF_MARGIN_DIV=0.5)
bpy.ops.uv.lightmap_pack(PREF_PACK_IN_ONE=False, PREF_NEW_UVLAYER=True)
