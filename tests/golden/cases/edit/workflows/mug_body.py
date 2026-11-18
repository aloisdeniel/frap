# WP 2.13 workflow (mug): a cylinder whose top cap (picked by its normal) is inset to leave a
# rim and extruded down into a cavity; then the whole body is shaded smooth and the normals
# recalculated outside.
# golden: startup=empty compare=objects,context,mesh:Mug tier=T1 order=canonical
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=24, radius=0.5, depth=1, location=(0, 0, 0.5))
bpy.context.object.name = 'Mug'
bpy.context.object.data.name = 'Mug'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_by_normal(direction=(0, 0, 1))
bpy.ops.mesh.inset(thickness=0.05)
bpy.ops.mesh.extrude_region_move(TRANSFORM_OT_translate={"value": (0, 0, -0.9)})
bpy.data.meshes['Mug'].total_face_sel
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.normals_make_consistent(inside=False)
bpy.ops.mesh.faces_shade_smooth()
