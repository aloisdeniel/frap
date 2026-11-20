# WP 6.2, Armature/Bone/BoneCollection RNA: bpy.data.armatures.new/remove (removing the
# armature removes its objects), an armature object made with objects.new (no pose until
# its mode changes), bones.active (selects the bone; another armature's bone is ignored),
# Bone.hide/hide_select deselect, bone collections (collections_all, roots, active,
# active_index, active_name, is_visible, is_solo, is_visible_effectively, bones, children,
# rename with unique names, bones keep their membership), Bone.name rename (unique, pose
# follows), the display settings, read-only and enum errors.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects.new('C0', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['C0'])
bpy.data.objects.new('C1', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['C1'])
bpy.data.armatures.new('Rig')
bpy.data.armatures['Rig'].bones
bpy.data.armatures['Rig'].collections_all
bpy.data.armatures['Rig'].display_type
bpy.data.armatures['Rig'].pose_position
bpy.data.armatures['Rig'].is_editmode
bpy.data.objects.new('RigOb', bpy.data.armatures['Rig'])
bpy.context.scene.collection.objects.link(bpy.data.objects['RigOb'])
bpy.data.objects['RigOb'].type
bpy.data.armatures['Rig'].edit_bones.new('x')
bpy.ops.object.armature_add()
bpy.data.armatures['Armature'].display_type = 'STICK'
bpy.data.armatures['Armature'].display_type = 'BOGUS'
bpy.data.armatures['Armature'].show_names = True
bpy.data.armatures['Armature'].show_axes = True
bpy.data.armatures['Armature'].axes_position = 0.5
bpy.data.objects['C0'].location.x = bpy.data.armatures['Armature'].axes_position
bpy.data.objects['C0'].location.y = bpy.data.armatures['Armature'].show_names
bpy.data.armatures['Armature'].bones['Bone'].head_local = (1, 1, 1)
bpy.data.armatures['Armature'].bones['Bone'].use_connect = True
bpy.data.armatures['Armature'].bones['Bone'].length = 3
bpy.data.armatures['Armature'].bones['Bone'].envelope_distance = 0.5
bpy.data.armatures['Armature'].bones['Bone'].inherit_scale = 'AVERAGE'
bpy.data.armatures['Armature'].bones['Bone'].inherit_scale = 'NOPE'
bpy.data.objects['C0'].location.z = bpy.data.armatures['Armature'].bones['Bone'].envelope_distance
bpy.data.armatures['Armature'].bones.active = None
bpy.data.armatures['Armature'].bones.active
bpy.data.armatures['Armature'].bones.active = bpy.data.armatures['Armature'].bones['Bone']
bpy.data.armatures['Armature'].bones.active.name
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.objects['C0'].rotation_euler.x = bpy.data.armatures['Armature'].edit_bones['Bone'].select
bpy.ops.armature.extrude_move(TRANSFORM_OT_translate={'value': (0, 1, 0)})
bpy.data.armatures['Armature'].collections.active_index
bpy.data.armatures['Armature'].collections.active_name
bpy.data.armatures['Armature'].edit_bones['Bone.001'].collections[0].name
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.armatures['Armature'].collections_all['Bones'].bones[1].name
bpy.data.armatures['Armature'].collections_all['Bones'].children
bpy.data.armatures['Armature'].collections_all['Bones'].parent
bpy.data.armatures['Armature'].collections_all['Bones'].index
bpy.data.armatures['Armature'].bones['Bone'].collections[0].name
bpy.data.armatures['Armature'].collections_all['Bones'].name = ''
bpy.data.armatures['Armature'].collections_all['Bones'].name = 'Deform'
bpy.data.armatures['Armature'].bones['Bone.001'].collections['Deform'].name
bpy.data.armatures['Armature'].collections.active_name
bpy.data.armatures['Armature'].collections['Deform'].is_visible = False
bpy.data.objects['C0'].rotation_euler.y = bpy.data.armatures['Armature'].collections['Deform'].is_visible_effectively
bpy.data.objects['C0'].rotation_euler.z = bpy.data.armatures['Armature'].collections['Deform'].is_visible_ancestors
bpy.ops.object.mode_set(mode='POSE')
bpy.ops.pose.select_all(action='SELECT')
bpy.context.selected_pose_bones[0]
bpy.data.armatures['Armature'].collections['Deform'].is_visible = True
bpy.data.armatures['Armature'].collections['Deform'].is_solo = True
bpy.data.armatures['Armature'].collections.is_solo_active
bpy.ops.pose.select_all(action='SELECT')
bpy.context.selected_pose_bones[1].name
bpy.data.armatures['Armature'].collections.active_index = 5
bpy.data.armatures['Armature'].collections.active
bpy.data.armatures['Armature'].collections.active_name = 'Deform'
bpy.data.armatures['Armature'].collections.active.name
bpy.data.armatures['Armature'].collections['Deform'].is_solo = False
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.armatures['Armature'].bones['Bone.001'].hide_select = True
bpy.data.objects['C1'].location.x = bpy.data.armatures['Armature'].bones['Bone.001'].hide_select
bpy.data.armatures['Armature'].bones['Bone.001'].name = 'Bone'
bpy.data.armatures['Armature'].bones['Bone.002'].name
bpy.data.objects['Armature'].pose.bones['Bone.002'].name
bpy.data.armatures['Armature'].bones['Bone.002'].name = 'Tip'
bpy.data.objects['Armature'].pose.bones['Tip'].bone.name
bpy.data.objects['Armature'].pose.bones['Tip'].parent.name
bpy.data.armatures['Armature'].bones['Tip'].parent.name
bpy.data.armatures['Armature'].bones['Bone'].children[0].name
bpy.data.armatures['Armature'].bones['Bone'].children['Tip'].name
bpy.data.objects['C1'].location.y = bpy.data.armatures['Armature'].bones['Tip'].use_connect
bpy.context.view_layer.objects.active = bpy.data.objects['RigOb']
bpy.ops.object.mode_set(mode='POSE')
bpy.data.objects['RigOb'].pose.bones
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.armatures.remove(bpy.data.armatures['Armature'])
bpy.data.objects['Armature']
bpy.data.armatures.remove(bpy.data.armatures['Rig'], do_unlink=False)
bpy.data.armatures['Rig'].name
