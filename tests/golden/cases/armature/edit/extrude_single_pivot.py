# WP 6.2, transform_around_single_fallback in armature Edit Mode: after
# armature.extrude_move only the new tip is selected (one TransData), so a rotate about the
# median, bounds or active element turns it about its bone's head (local origins) and a
# resize scales it about itself; the cursor pivot and center_override keep their center;
# every orientation goes through the same fallback. Proportional editing changes nothing
# (every armature TransData is selected).
# golden: startup=factory compare=objects,context tier=T0 tol=1e-5
import bpy
bpy.data.objects.new('C0', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['C0'])
bpy.data.objects.new('C1', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['C1'])
bpy.data.objects.new('C2', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['C2'])
bpy.data.objects.new('C3', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['C3'])
bpy.data.objects.new('C4', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['C4'])
bpy.data.objects.new('C5', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['C5'])
bpy.data.objects.new('C6', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['C6'])
bpy.data.objects.new('C7', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['C7'])
bpy.ops.object.armature_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.armature.extrude_move(TRANSFORM_OT_translate={'value': (0, 0, 1)})
bpy.ops.transform.rotate(value=0.6, orient_axis='Y')
bpy.data.objects['C0'].location = bpy.data.armatures['Armature'].edit_bones['Bone.001'].tail
bpy.data.objects['C0'].rotation_euler.x = bpy.data.armatures['Armature'].edit_bones['Bone.001'].roll
bpy.data.objects['C0'].rotation_euler.y = bpy.data.armatures['Armature'].edit_bones['Bone.001'].head_radius
bpy.data.objects['C0'].rotation_euler.z = bpy.data.armatures['Armature'].edit_bones['Bone.001'].tail_radius
bpy.data.objects['C0'].scale = bpy.data.armatures['Armature'].edit_bones['Bone.001'].head
bpy.ops.transform.rotate(value=0.3, orient_axis='X', orient_type='LOCAL')
bpy.ops.transform.rotate(value=0.2, orient_axis='Z', orient_type='NORMAL')
bpy.data.objects['C1'].location = bpy.data.armatures['Armature'].edit_bones['Bone.001'].tail
bpy.data.objects['C1'].rotation_euler.x = bpy.data.armatures['Armature'].edit_bones['Bone.001'].roll
bpy.data.objects['C1'].rotation_euler.y = bpy.data.armatures['Armature'].edit_bones['Bone.001'].head_radius
bpy.data.objects['C1'].rotation_euler.z = bpy.data.armatures['Armature'].edit_bones['Bone.001'].tail_radius
bpy.data.objects['C1'].scale = bpy.data.armatures['Armature'].edit_bones['Bone.001'].head
bpy.ops.transform.resize(value=(2, 2, 2))
bpy.ops.transform.resize(value=(1, 1.5, 1), orient_type='NORMAL', constraint_axis=(False, True, False))
bpy.data.objects['C2'].location = bpy.data.armatures['Armature'].edit_bones['Bone.001'].tail
bpy.data.objects['C2'].rotation_euler.x = bpy.data.armatures['Armature'].edit_bones['Bone.001'].roll
bpy.data.objects['C2'].rotation_euler.y = bpy.data.armatures['Armature'].edit_bones['Bone.001'].head_radius
bpy.data.objects['C2'].rotation_euler.z = bpy.data.armatures['Armature'].edit_bones['Bone.001'].tail_radius
bpy.data.objects['C2'].scale = bpy.data.armatures['Armature'].edit_bones['Bone.001'].head
bpy.ops.transform.rotate(value=0.4, orient_axis='Z', center_override=(0, 0, 0))
bpy.data.objects['C3'].location = bpy.data.armatures['Armature'].edit_bones['Bone.001'].tail
bpy.data.objects['C3'].rotation_euler.x = bpy.data.armatures['Armature'].edit_bones['Bone.001'].roll
bpy.data.objects['C3'].rotation_euler.y = bpy.data.armatures['Armature'].edit_bones['Bone.001'].head_radius
bpy.data.objects['C3'].rotation_euler.z = bpy.data.armatures['Armature'].edit_bones['Bone.001'].tail_radius
bpy.data.objects['C3'].scale = bpy.data.armatures['Armature'].edit_bones['Bone.001'].head
bpy.context.scene.tool_settings.transform_pivot_point = 'ACTIVE_ELEMENT'
bpy.ops.transform.rotate(value=0.5, orient_axis='X', orient_type='VIEW')
bpy.context.scene.tool_settings.transform_pivot_point = 'BOUNDING_BOX_CENTER'
bpy.ops.transform.rotate(value=-0.25, orient_axis='Y', orient_type='CURSOR')
bpy.data.objects['C4'].location = bpy.data.armatures['Armature'].edit_bones['Bone.001'].tail
bpy.data.objects['C4'].rotation_euler.x = bpy.data.armatures['Armature'].edit_bones['Bone.001'].roll
bpy.data.objects['C4'].rotation_euler.y = bpy.data.armatures['Armature'].edit_bones['Bone.001'].head_radius
bpy.data.objects['C4'].rotation_euler.z = bpy.data.armatures['Armature'].edit_bones['Bone.001'].tail_radius
bpy.data.objects['C4'].scale = bpy.data.armatures['Armature'].edit_bones['Bone.001'].head
bpy.context.scene.tool_settings.transform_pivot_point = 'CURSOR'
bpy.ops.transform.rotate(value=0.5, orient_axis='X')
bpy.data.objects['C5'].location = bpy.data.armatures['Armature'].edit_bones['Bone.001'].tail
bpy.data.objects['C5'].rotation_euler.x = bpy.data.armatures['Armature'].edit_bones['Bone.001'].roll
bpy.data.objects['C5'].rotation_euler.y = bpy.data.armatures['Armature'].edit_bones['Bone.001'].head_radius
bpy.data.objects['C5'].rotation_euler.z = bpy.data.armatures['Armature'].edit_bones['Bone.001'].tail_radius
bpy.data.objects['C5'].scale = bpy.data.armatures['Armature'].edit_bones['Bone.001'].head
bpy.context.scene.tool_settings.transform_pivot_point = 'MEDIAN_POINT'
bpy.ops.armature.extrude_move(TRANSFORM_OT_translate={'value': (0.5, 0, 0)})
bpy.ops.transform.rotate(value=0.7, orient_axis='Z', orient_type='GIMBAL')
bpy.ops.transform.rotate(value=0.1, orient_axis='X', orient_type='PARENT')
bpy.data.objects['C6'].location = bpy.data.armatures['Armature'].edit_bones['Bone.002'].tail
bpy.data.objects['C6'].rotation_euler.x = bpy.data.armatures['Armature'].edit_bones['Bone.002'].roll
bpy.data.objects['C6'].rotation_euler.y = bpy.data.armatures['Armature'].edit_bones['Bone.002'].head_radius
bpy.data.objects['C6'].rotation_euler.z = bpy.data.armatures['Armature'].edit_bones['Bone.002'].tail_radius
bpy.data.objects['C6'].scale = bpy.data.armatures['Armature'].edit_bones['Bone.002'].head
bpy.data.objects['C7'].location = bpy.data.armatures['Armature'].edit_bones['Bone.001'].tail
bpy.data.objects['C7'].rotation_euler.x = bpy.data.armatures['Armature'].edit_bones['Bone.001'].roll
bpy.data.objects['C7'].rotation_euler.y = bpy.data.armatures['Armature'].edit_bones['Bone.001'].head_radius
bpy.data.objects['C7'].rotation_euler.z = bpy.data.armatures['Armature'].edit_bones['Bone.001'].tail_radius
bpy.data.objects['C7'].scale = bpy.data.armatures['Armature'].edit_bones['Bone.001'].head
bpy.ops.armature.select_all(action='SELECT')
bpy.ops.transform.translate(value=(0, 0.5, 0.25), use_proportional_edit=True, proportional_size=0.2)
bpy.ops.transform.rotate(value=0.3, orient_axis='Z', use_proportional_edit=True, proportional_edit_falloff='SHARP')
bpy.data.objects['C6'].scale = bpy.data.armatures['Armature'].edit_bones['Bone.002'].tail
bpy.data.objects['C7'].scale = bpy.data.armatures['Armature'].edit_bones['Bone'].head
bpy.ops.object.mode_set(mode='OBJECT')
