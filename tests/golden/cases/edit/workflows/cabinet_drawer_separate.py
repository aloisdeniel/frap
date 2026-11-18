# WP 2.13 workflow (cabinet): the front face of a box (picked by its normal) is inset and
# pushed in as a recess; the recess floor is duplicated, moved out as the drawer front, given
# thickness with mesh.solidify and separated into its own object.
# golden: startup=empty compare=objects,context,mesh:Cabinet,mesh:Cabinet.001 tier=T1 order=canonical
import bpy
bpy.ops.mesh.primitive_cube_add(size=1, location=(0, 0, 0.5), scale=(0.8, 0.5, 1))
bpy.context.object.name = 'Cabinet'
bpy.context.object.data.name = 'Cabinet'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_by_normal(direction=(0, -1, 0))
bpy.ops.mesh.inset(thickness=0.04, depth=-0.3)
bpy.ops.mesh.duplicate_move(TRANSFORM_OT_translate={"value": (0, -0.28, 0)})
bpy.data.meshes['Cabinet'].total_face_sel
bpy.ops.mesh.solidify(thickness=0.02)
bpy.ops.mesh.select_linked()
bpy.data.meshes['Cabinet'].total_face_sel
bpy.ops.mesh.separate(type='SELECTED')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.selected_objects
