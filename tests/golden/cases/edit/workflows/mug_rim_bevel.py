# WP 2.13 workflow (mug): the mug body (inset + extrude), then the faces facing up (the rim
# ring and the cavity floor) are selected by normal, reduced to their boundary loops with
# mesh.region_to_loop, and those loops bevelled with two segments to round the rim and the
# inner floor.
# golden: startup=empty compare=objects,context,mesh:Mug tier=T2 tol=1e-4
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=16, radius=0.5, depth=1, location=(0, 0, 0.5))
bpy.context.object.name = 'Mug'
bpy.context.object.data.name = 'Mug'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_by_normal(direction=(0, 0, 1))
bpy.ops.mesh.inset(thickness=0.06)
bpy.ops.mesh.extrude_region_move(TRANSFORM_OT_translate={"value": (0, 0, -0.85)})
bpy.ops.frap.select_by_normal(direction=(0, 0, 1), threshold=0.1)
bpy.data.meshes['Mug'].total_face_sel
bpy.ops.mesh.region_to_loop()
bpy.data.meshes['Mug'].total_edge_sel
bpy.ops.mesh.bevel(offset=0.015, segments=2)
bpy.data.meshes['Mug'].total_face_sel
