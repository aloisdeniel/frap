# WP 2.13 workflow (mug, preparing UVs): the seam recipe for a cylinder: one vertical edge plus
# both cap boundaries (each cap picked by its normal, reduced to its loop with
# mesh.region_to_loop) marked as seams; the caps' loops also marked sharp, then the side shaded
# smooth.
# golden: startup=empty compare=objects,context,mesh:Cylinder tier=T1
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=16, radius=0.4, depth=1)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[2])
bpy.ops.mesh.mark_seam()
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_by_normal(direction=(0, 0, 1), threshold=0.1)
bpy.ops.frap.select_by_normal(direction=(0, 0, -1), threshold=0.1, extend=True)
bpy.ops.mesh.region_to_loop()
bpy.data.meshes['Cylinder'].total_edge_sel
bpy.ops.mesh.mark_seam()
bpy.ops.mesh.mark_sharp()
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.faces_shade_smooth()
