# WP 6.5: the armature-parent virtual modifier. parent_type = 'ARMATURE' (Blender's PARSKEL)
# deforms a mesh child by its armature parent with vertex groups and envelopes, before its own
# modifiers and after its shape keys; parent_set(type='ARMATURE') still adds an Armature modifier
# (it sets parent_type OBJECT before looking for one, so the virtual modifier is gone), but not a
# second one for the same armature; RNA parent_type changes reset matrix_parent_inverse; a
# use_multi_modifier ARMATURE modifier after the virtual one and another armature's modifier;
# pose_position = 'REST' gives the rest shape.
# golden: startup=factory tier=T1 tol=1e-5 compare=objects,evaluated:Cube,evaluated:Cube.001
import bpy
bpy.ops.object.armature_add(location=(0, 0, -1.2))
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.armatures['Armature'].edit_bones['Bone'].tail = (0, 0, 1.0)
bpy.data.armatures['Armature'].edit_bones['Bone'].tail_radius = 0.6
bpy.data.armatures['Armature'].edit_bones['Bone'].head_radius = 0.6
bpy.data.armatures['Armature'].edit_bones['Bone'].envelope_distance = 1.0
bpy.data.armatures['Armature'].edit_bones.new('Top')
bpy.data.armatures['Armature'].edit_bones['Top'].head = (0, 0, 1.0)
bpy.data.armatures['Armature'].edit_bones['Top'].tail = (0, 0, 2.4)
bpy.data.armatures['Armature'].edit_bones['Top'].parent = bpy.data.armatures['Armature'].edit_bones['Bone']
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Cube'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.duplicate()
bpy.data.objects['Cube.001'].location = (0.3, 0.2, 0.0)
bpy.data.objects['Cube'].shape_key_add(name='Basis')
bpy.data.objects['Cube'].shape_key_add(name='Up')
bpy.data.shape_keys['Key'].key_blocks['Up'].data[0].co = (1.0, 1.0, 1.6)
bpy.data.shape_keys['Key'].key_blocks['Up'].value = 0.5
bpy.data.objects['Cube'].vertex_groups.new(name='Top')
bpy.data.objects['Cube'].vertex_groups['Top'].add([0, 2, 4, 6], 1.0, 'REPLACE')
bpy.data.objects['Cube'].modifiers.new('Bevel', 'BEVEL')
bpy.data.objects['Cube'].parent = bpy.data.objects['Armature']
bpy.data.objects['Cube'].parent_type = 'ARMATURE'
bpy.data.objects['Cube.001'].parent = bpy.data.objects['Armature']
bpy.data.objects['Cube.001'].parent_type = 'ARMATURE'
bpy.data.objects['Armature'].pose.bones['Bone'].rotation_quaternion = (0.95, 0.3, 0.0, 0.0)
bpy.data.objects['Armature'].pose.bones['Top'].rotation_quaternion = (0.9, 0.0, 0.0, 0.45)
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube.001'].select_set(True)
bpy.data.objects['Armature'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Armature']
bpy.ops.object.parent_set(type='ARMATURE')
bpy.data.objects['Cube.001'].parent_type
bpy.data.objects['Cube.001'].parent_type = 'ARMATURE'
bpy.ops.object.armature_add(location=(2, 0, 0))
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube.001'].select_set(True)
bpy.data.objects['Armature.001'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Armature.001']
bpy.ops.object.parent_set(type='ARMATURE')
bpy.data.objects['Cube.001'].parent = bpy.data.objects['Armature']
bpy.data.objects['Cube.001'].parent_type = 'ARMATURE'
bpy.data.objects['Cube.001'].modifiers.new('Second', 'ARMATURE')
bpy.data.objects['Cube.001'].modifiers['Second'].object = bpy.data.objects['Armature']
bpy.data.objects['Cube.001'].modifiers['Second'].use_multi_modifier = True
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube.001'].select_set(True)
bpy.data.objects['Armature'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Armature']
bpy.ops.object.parent_set(type='ARMATURE')
bpy.data.objects['Cube.001'].modifiers.keys()
#@blender assert bpy.data.objects['Cube.001'].modifiers.keys() == ['Armature', 'Armature.001', 'Second']
bpy.data.objects['Cube.001'].parent_type = 'ARMATURE'
bpy.data.armatures['Armature.001'].pose_position = 'REST'
