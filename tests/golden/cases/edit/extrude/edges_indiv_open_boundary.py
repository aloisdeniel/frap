# WP 2.5: mesh.extrude_edges_indiv on a grid's open boundary edge and on a wire edge made by a
# vertex extrusion (edge without a face). Two input edges: Blender's side-quad order is
# run-dependent (GHash), so compared order-free.
# golden: startup=empty compare=objects,context,mesh:Grid tier=T1 order=canonical
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=2, y_subdivisions=2, size=2)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[8])
bpy.ops.mesh.extrude_verts_indiv()
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 12])
bpy.ops.mesh.extrude_edges_indiv()
bpy.data.meshes['Grid'].total_edge_sel
