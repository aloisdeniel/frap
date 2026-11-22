# WP 8.3.7: `object.vertex_group_copy` (`_copy` names, the lock and the weights copied, the copy
# active), `vertex_group_lock` (every action and mask; outside Weight Paint Mode the "selected"
# group is the active one), `vertex_group_move` (UP/DOWN, nothing at the ends) and
# `vertex_group_sort` (natural, case-insensitive name order; UTF-8 names after ASCII ones) with
# weights and the active index following their groups, in Object and Edit Mode. Group order,
# locks and the active index are read into object transforms; weights through DISPLACE
# modifiers by group name.
# golden: startup=factory tier=T1 compare=objects,evaluated:Cube
import bpy
bpy.ops.object.vertex_group_copy()
bpy.ops.object.vertex_group_lock()
bpy.data.objects['Cube'].vertex_groups.new(name='Leg')
bpy.data.objects['Cube'].vertex_groups.new(name='arm10')
bpy.data.objects['Cube'].vertex_groups.new(name='Arm2')
bpy.data.objects['Cube'].vertex_groups.new(name='body')
bpy.ops.object.vertex_group_copy()
bpy.data.objects['Cube'].vertex_groups['Leg'].add([0, 1, 2], 0.25, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups['arm10'].add([3, 4], 0.5, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups['Arm2'].add([5], 0.125, 'REPLACE')
bpy.data.objects['Cube'].vertex_groups['body'].add([1, 6, 7], 1.0, 'REPLACE')
bpy.ops.object.vertex_group_set_active(group='Leg')
bpy.data.objects['Cube'].vertex_groups['Leg'].lock_weight = True
bpy.ops.object.vertex_group_copy()
bpy.ops.object.vertex_group_copy()
bpy.data.objects['Camera'].location = (bpy.data.objects['Cube'].vertex_groups['Leg_copy'].index, bpy.data.objects['Cube'].vertex_groups['Leg_copy.001'].index, bpy.data.objects['Cube'].vertex_groups.active_index)
bpy.data.objects['Camera'].scale = (bpy.data.objects['Cube'].vertex_groups['Leg_copy'].lock_weight, bpy.data.objects['Cube'].vertex_groups['Leg_copy.001'].weight(2), bpy.data.objects['Cube'].vertex_groups['body'].index + 1)
bpy.data.objects['Cube'].vertex_groups['Leg_copy.001'].weight(3)
bpy.ops.object.vertex_group_lock(action='TOGGLE')
bpy.data.objects['Camera'].rotation_euler = (bpy.data.objects['Cube'].vertex_groups['Leg'].lock_weight, bpy.data.objects['Cube'].vertex_groups['arm10'].lock_weight, bpy.data.objects['Cube'].vertex_groups['Leg_copy.001'].lock_weight)
bpy.ops.object.vertex_group_lock(action='TOGGLE')
bpy.ops.object.vertex_group_set_active(group='Arm2')
bpy.ops.object.vertex_group_lock(action='LOCK', mask='SELECTED')
bpy.ops.object.vertex_group_lock(action='INVERT', mask='UNSELECTED')
bpy.data.objects['Light'].location = (bpy.data.objects['Cube'].vertex_groups['Arm2'].lock_weight, bpy.data.objects['Cube'].vertex_groups['Leg'].lock_weight, bpy.data.objects['Cube'].vertex_groups['body'].lock_weight)
bpy.ops.object.vertex_group_lock(action='UNLOCK', mask='INVERT_UNSELECTED')
bpy.data.objects['Light'].rotation_euler = (bpy.data.objects['Cube'].vertex_groups['Arm2'].lock_weight, bpy.data.objects['Cube'].vertex_groups['Leg'].lock_weight, bpy.data.objects['Cube'].vertex_groups['body'].lock_weight)
bpy.ops.object.vertex_group_lock(action='TOGGLE', mask='INVERT_UNSELECTED')
bpy.data.objects['Light'].scale = (bpy.data.objects['Cube'].vertex_groups['Arm2'].lock_weight, bpy.data.objects['Cube'].vertex_groups['Leg'].lock_weight, bpy.data.objects['Cube'].vertex_groups['body'].lock_weight)
bpy.ops.object.vertex_group_lock(action='UNLOCK')
bpy.ops.object.vertex_group_move(direction='UP')
bpy.ops.object.vertex_group_move(direction='UP')
bpy.ops.object.vertex_group_move(direction='UP')
bpy.ops.object.vertex_group_move(direction='DOWN')
bpy.data.objects['Camera'].delta_location = (bpy.data.objects['Cube'].vertex_groups['Arm2'].index, bpy.data.objects['Cube'].vertex_groups['Leg'].index, bpy.data.objects['Cube'].vertex_groups.active_index)
bpy.data.objects['Camera'].delta_scale = (bpy.data.objects['Cube'].vertex_groups['Arm2'].weight(5), bpy.data.objects['Cube'].vertex_groups['arm10'].weight(4), bpy.data.objects['Cube'].vertex_groups['arm10'].index)
bpy.ops.object.vertex_group_set_active(group='Leg_copy.001')
bpy.ops.object.vertex_group_move(direction='DOWN')
bpy.data.objects['Cube'].vertex_groups.new(name='Ärm_ünï')
bpy.data.objects['Cube'].vertex_groups['Ärm_ünï'].add([7], 0.375, 'REPLACE')
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.object.vertex_group_sort(sort_type='NAME')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Light'].delta_location = (bpy.data.objects['Cube'].vertex_groups['Arm2'].index, bpy.data.objects['Cube'].vertex_groups['arm10'].index, bpy.data.objects['Cube'].vertex_groups['body'].index)
bpy.data.objects['Light'].delta_scale = (bpy.data.objects['Cube'].vertex_groups['Leg'].index, bpy.data.objects['Cube'].vertex_groups['Leg_copy'].index, bpy.data.objects['Cube'].vertex_groups['Leg_copy.001'].index)
bpy.data.objects['Camera'].delta_rotation_euler = (bpy.data.objects['Cube'].vertex_groups.active_index, bpy.data.objects['Cube'].vertex_groups['body'].weight(6), bpy.data.objects['Cube'].vertex_groups['Leg_copy.001'].weight(0))
bpy.data.objects['Light'].delta_rotation_euler = (bpy.data.objects['Cube'].vertex_groups['Ärm_ünï'].index, bpy.data.objects['Cube'].vertex_groups['Ärm_ünï'].weight(7), bpy.data.objects['Cube'].vertex_groups['arm10'].index)
bpy.ops.object.vertex_group_sort(sort_type='BONE_HIERARCHY')
bpy.data.objects['Cube'].modifiers.new(name='A', type='DISPLACE')
bpy.data.objects['Cube'].modifiers['A'].vertex_group = 'Leg_copy.001'
bpy.data.objects['Cube'].modifiers['A'].direction = 'X'
bpy.data.objects['Cube'].modifiers.new(name='B', type='DISPLACE')
bpy.data.objects['Cube'].modifiers['B'].vertex_group = 'arm10'
bpy.data.objects['Cube'].modifiers['B'].direction = 'Y'
bpy.data.objects['Cube'].modifiers.new(name='C', type='DISPLACE')
bpy.data.objects['Cube'].modifiers['C'].vertex_group = 'body'
bpy.data.objects['Cube'].modifiers['C'].direction = 'Z'
