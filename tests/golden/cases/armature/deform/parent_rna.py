# WP 6.5: Object.parent_type and Object.parent_bone assignment (ed::object::parent_set). The
# parent_type items follow the parent (OBJECT; ARMATURE and BONE for an armature; VERTEX and
# VERTEX_3 for a mesh), another value is a TypeError listing them; the current type changes
# nothing, another one resets matrix_parent_inverse; parent_bone resets it too; without a
# parent both clear the parent data. A child of a bone then follows the bone.
# golden: startup=factory tier=T0 tol=1e-5 compare=objects
import bpy
bpy.data.objects['Cube'].parent_type = 'BONE'
bpy.data.objects['Cube'].parent_type = 'OBJECT'
bpy.data.objects['Cube'].parent_bone = 'X'
bpy.data.objects['Cube'].parent_bone
#@blender assert bpy.data.objects['Cube'].parent_bone == ''
bpy.ops.object.armature_add(location=(0.5, 0.0, 0.5), rotation=(0.2, 0.0, 0.3))
bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion = (0.9, 0.3, 0.1, 0.0)
bpy.data.objects['Cube'].parent = bpy.data.objects['Light']
bpy.data.objects['Cube'].matrix_parent_inverse = bpy.data.objects['Light'].matrix_world.inverted()
bpy.data.objects['Cube'].parent_type = 'BONE'
bpy.data.objects['Cube'].parent_type = 'OBJECT'
bpy.data.objects['Cube'].parent_type = 'VERTEX'
bpy.data.objects['Cube'].parent_bone = 'Y'
bpy.data.objects['Cube'].matrix_parent_inverse
bpy.data.objects['Camera'].parent = bpy.data.objects['Cube']
bpy.data.objects['Camera'].parent_type = 'VERTEX_3'
bpy.data.objects['Camera'].parent_type = 'ARMATURE'
bpy.data.objects['Camera'].parent_type = 'OBJECT'
bpy.data.objects['Cube'].parent = bpy.data.objects['Armature']
bpy.data.objects['Cube'].matrix_parent_inverse = bpy.data.objects['Light'].matrix_world.inverted()
bpy.data.objects['Cube'].parent_type = 'BONE'
bpy.data.objects['Cube'].matrix_parent_inverse = bpy.data.objects['Light'].matrix_world.inverted()
bpy.data.objects['Cube'].parent_type = 'BONE'
bpy.data.objects['Cube'].parent_type = 'LATTICE'
bpy.data.objects['Cube'].parent_bone = 'Bone'
bpy.data.objects['Cube'].parent_type
bpy.data.objects['Light'].parent = bpy.data.objects['Armature']
bpy.data.objects['Light'].parent_type = 'ARMATURE'
bpy.data.objects['Light'].parent_bone = 'Bone'
bpy.data.objects['Light'].parent_type = 'BONE'
bpy.data.objects['Light'].parent = None
bpy.data.objects['Light'].parent_type
bpy.data.objects['Light'].parent_bone
