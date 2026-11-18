# WP 2.10: mesh.screw (needs extrude_face_region, WP 2.5): an open chain of three vertices
# (two edges of a circle) screwed 8 steps around Z, then two turns of a tilted chain; the
# chain's end-to-end offset gives the helix pitch; the last step is selected.
# golden: startup=factory compare=objects,context,mesh:Circle tier=T2
import bpy
bpy.ops.mesh.primitive_circle_add(vertices=8, radius=1, location=(0, 0, 0), enter_editmode=True)
bpy.ops.frap.select_elements(type='VERT', indices=[0, 1, 2])
bpy.ops.mesh.screw(steps=8, turns=1, center=(0, 0, 0), axis=(0, 0, 1))
bpy.data.meshes['Circle'].total_vert_sel
bpy.ops.frap.select_elements(type='VERT', indices=[4, 5])
bpy.ops.mesh.screw(steps=5, turns=2, center=(0.5, 0, 0), axis=(0, 0.6, 0.8))
bpy.data.meshes['Circle'].total_vert_sel
