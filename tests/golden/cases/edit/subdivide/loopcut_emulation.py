# WP 2.6: mesh.loopcut(edge_index=…) equals the golden-harness emulation of
# docs/research/blender-ops-mesh-uv.md App. B.7 (ring walk + bmesh.ops.subdivide_edges with PATH
# corners, grid fill, smooth-even, then only the new inner edges selected), in edge select mode:
# the cube's ring of edge 0 cut twice (V16 E28 F14, edges 20-27 selected).
# golden: startup=factory compare=objects,context,mesh:Cube.001 tier=T1
import bpy
bpy.ops.mesh.primitive_cube_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
#@frap    bpy.ops.mesh.loopcut(number_cuts=2, edge_index=0, object_index=0)
#@blender import bmesh; bpy.ops.mesh.select_all(action='DESELECT'); me = bpy.context.object.data; bm = bmesh.from_edit_mesh(me); bm.edges.ensure_lookup_table(); bm.edges[0].select_set(True); bmesh.update_edit_mesh(me); bpy.ops.mesh.select_edge_ring_multi()
#@blender bm = bmesh.from_edit_mesh(me); ring = [e for e in bm.edges if e.select]; r = bmesh.ops.subdivide_edges(bm, edges=ring, cuts=2, smooth=0.0, smooth_falloff='INVERSE_SQUARE', use_smooth_even=True, quad_corner_type='PATH', use_single_edge=True, use_grid_fill=True, use_only_quads=True)
#@blender for x in list(bm.verts) + list(bm.edges) + list(bm.faces): x.select = False
#@blender bm.select_history.clear(); [e.select_set(True) for e in r['geom_inner'] if isinstance(e, bmesh.types.BMEdge)]; bm.select_flush_mode(); bmesh.update_edit_mesh(me)
bpy.data.meshes['Cube.001'].total_edge_sel
