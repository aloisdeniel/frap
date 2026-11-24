# M8 stabilization round 8: `foreach_get` / `foreach_set` copy a MATRIX property in Blender's
# memory order, column by column (the translation is at items 12-14 of a 4x4), on `bpy.data`
# collections, ID collections served item by item (`scene.objects`), bones and pose bones
# (round 7 flattened them row by row: `foreach_set('matrix_basis', …)` dropped the
# translation). `foreach_set` on ID collections bound as lists (`collection.objects`,
# `collection.children`, `mesh.materials`) and on `pose.bones` writes item by item (round 7
# raised TypeError). Blender's `foreach_set` does not tag the depsgraph, so `matrix_world`
# stays stale until a plain write (D-175): the last lines rewrite each location.
# golden: startup=factory tier=T0 compare=objects
import bpy
bpy.data.objects.foreach_get('matrix_world', [0.0] * 48)
bpy.data.scenes['Scene'].objects.foreach_get('matrix_world', [0.0] * 48)
bpy.data.objects.foreach_get('matrix_world', [0.0] * 47)
bpy.data.objects.foreach_set('matrix_basis', [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 5, 6, 7, 1] * 3)
bpy.data.objects['Cube'].location
bpy.data.scenes['Scene'].objects.foreach_set('matrix_basis', [0, 1, 0, 0, -1, 0, 0, 0, 0, 0, 1, 0, 1, 2, 3, 1] * 3)
bpy.data.collections['Collection'].objects.foreach_set('location', [2.0] * 9)
bpy.data.collections['Collection'].objects.foreach_set('scale', [2.0] * 9)
bpy.data.scenes['Scene'].collection.children.foreach_set('hide_render', [True])
bpy.data.scenes['Scene'].collection.children.foreach_set('hide_select', [1])
bpy.data.meshes['Cube'].materials.foreach_set('roughness', [0.1])
bpy.ops.object.armature_add(location=(1, 2, 3))
bpy.data.armatures['Armature'].bones.foreach_get('matrix', [0.0] * 9)
bpy.data.armatures['Armature'].bones.foreach_get('matrix_local', [0.0] * 16)
bpy.data.objects['Armature'].pose.bones.foreach_get('matrix', [0.0] * 16)
bpy.data.objects['Armature'].pose.bones.foreach_set('location', [1.0] * 3)
bpy.data.objects['Armature'].pose.bones.foreach_set('rotation_quaternion', [1.0, 0.5, 0, 0])
bpy.data.objects['Armature'].pose.bones.foreach_set('matrix_basis', [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 5, 6, 7, 1])
bpy.data.objects['Armature'].pose.bones.foreach_set('location', [1.0] * 4)
bpy.data.objects['Armature'].pose.bones.foreach_set('location', [1.0] * 2)
bpy.data.objects['Camera'].location = (2, 2, 2)
bpy.data.objects['Cube'].location = (2, 2, 2)
bpy.data.objects['Light'].location = (2, 2, 2)
