# WP 6.1: ActionSlots.active, ActionSlot.duplicate() (flags and channels copied, identifier
# made unique), ActionSlot.users() (called), identifier / name_display / target_id_type writes, and the
# plain properties of slots, layers, groups, F-curves and keys (moved into object names and
# transforms so they are compared).
# golden: startup=factory tier=T1 tol=1e-5 compare=anim:S,objects
import bpy
bpy.data.actions.new('S')
bpy.data.actions['S'].slots.new(id_type='OBJECT', name='Cube')
bpy.data.actions['S'].layers.new('Layer').strips.new(type='KEYFRAME').channelbag(bpy.data.actions['S'].slots[0], ensure=True).fcurves.new('location', index=0, group_name='Grp')
bpy.data.actions['S'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points.insert(1, 0.0)
bpy.data.actions['S'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points.insert(7, 3.0)
bpy.data.actions['S'].slots.active = bpy.data.actions['S'].slots[0]
bpy.data.actions['S'].slots[0].select = True
bpy.data.actions['S'].slots[0].duplicate()
bpy.data.objects.new('dup_' + bpy.data.actions['S'].slots[1].identifier, None)
bpy.data.objects.new('active_' + bpy.data.actions['S'].slots.active.identifier, None)
bpy.data.objects['Camera'].location = (bpy.data.actions['S'].slots[1].active, bpy.data.actions['S'].slots[1].select, bpy.data.actions['S'].slots[1].handle - 929201140)
bpy.data.actions['S'].slots.active = None
bpy.data.actions['S'].slots.active.identifier
bpy.data.actions['S'].slots[1].show_expanded = False
bpy.data.actions['S'].slots[0].target_id_type = 'MATERIAL'
bpy.data.objects.new(bpy.data.actions['S'].slots[0].target_id_type, None)
bpy.data.objects['Cube'].animation_data_create()
bpy.data.objects['Cube'].animation_data.action = bpy.data.actions['S']
bpy.data.actions['S'].slots[0].users()
bpy.data.actions['S'].slots[1].users()
bpy.data.actions['S'].slots[0].identifier = 'MAWrong'
bpy.data.objects.new('id_' + bpy.data.actions['S'].slots[0].identifier, None)
bpy.data.objects.new('last_' + bpy.data.objects['Cube'].animation_data.last_slot_identifier, None)
bpy.data.actions['S'].slots[0].identifier = 'OBCube.001'
bpy.data.objects.new('id2_' + bpy.data.actions['S'].slots[0].identifier, None)
bpy.data.actions['S'].slots[1].name_display = 'Display'
bpy.data.objects.new('disp_' + bpy.data.actions['S'].slots[1].identifier + '_' + bpy.data.actions['S'].slots[1].name_display, None)
bpy.data.actions['S'].layers[0].name = 'Renamed'
bpy.data.objects.new(bpy.data.actions['S'].layers['Renamed'].strips[0].type, None)
bpy.data.actions['S'].layers[0].strips[0].channelbags[0].groups['Grp'].name = 'NewName'
bpy.data.objects.new('group_' + bpy.data.actions['S'].layers[0].strips[0].channelbags[0].fcurves[0].group.name, None)
bpy.data.objects.new('channel_' + bpy.data.actions['S'].layers[0].strips[0].channelbags[0].groups[0].channels[0].data_path, None)
bpy.data.actions['S'].layers[0].strips[0].channelbags[0].groups[0].mute = True
bpy.data.actions['S'].layers[0].strips[0].channelbags[0].groups[0].lock = True
bpy.data.actions['S'].layers[0].strips[0].channelbags[0].groups[0].color_set = 'THEME03'
bpy.data.objects['Camera'].scale = (bpy.data.actions['S'].layers[0].strips[0].channelbags[0].groups[0].select, bpy.data.actions['S'].layers[0].strips[0].channelbags[0].groups[0].is_custom_color_set, bpy.data.actions['S'].layers[0].strips[0].channelbags[0].groups[0].show_expanded)
bpy.data.objects.new(bpy.data.actions['S'].layers[0].strips[0].channelbags[0].groups[0].color_set, None)
bpy.data.actions['S'].layers[0].strips[0].channelbags[0].fcurves[0].mute = True
bpy.data.actions['S'].layers[0].strips[0].channelbags[0].fcurves[0].hide = True
bpy.data.actions['S'].layers[0].strips[0].channelbags[0].fcurves[0].lock = True
bpy.data.actions['S'].layers[0].strips[0].channelbags[0].fcurves[0].color_mode = 'AUTO_RGB'
bpy.data.actions['S'].layers[0].strips[0].channelbags[0].fcurves[0].array_index = 2
bpy.data.actions['S'].layers[0].strips[0].channelbags[0].fcurves[0].data_path = 'delta_location'
bpy.data.objects['Light'].location = (bpy.data.actions['S'].layers[0].strips[0].channelbags[0].fcurves[0].mute, bpy.data.actions['S'].layers[0].strips[0].channelbags[0].fcurves[0].is_empty, bpy.data.actions['S'].layers[0].strips[0].channelbags[0].fcurves[0].select)
bpy.data.objects['Light'].scale = (bpy.data.actions['S'].layers[0].strips[0].channelbags[0].fcurves[0].is_valid, bpy.data.actions['S'].layers[0].strips[0].channelbags[0].slot_handle - 929201140, bpy.data.actions['S'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[1].select_control_point)
bpy.data.objects.new(bpy.data.actions['S'].layers[0].strips[0].channelbags[0].fcurves[0].color_mode + bpy.data.actions['S'].layers[0].strips[0].channelbags[0].fcurves[0].auto_smoothing, None)
bpy.data.actions['S'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[1].type = 'EXTREME'
bpy.data.objects.new(bpy.data.actions['S'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[1].type + bpy.data.actions['S'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[1].handle_left_type, None)
bpy.data.actions['S'].deselect_keys()
bpy.data.objects['Camera'].rotation_euler = (bpy.data.actions['S'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[0].select_control_point, bpy.data.actions['S'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points[0].select_left_handle, bpy.data.actions['S'].layers[0].strips[0].channelbags[0].fcurves[0].select)
bpy.data.actions['S'].layers[0].strips[0].channelbags[0].fcurves[0].keyframe_points.foreach_set('co', [2.0, 1.0, 9.0, -1.0])
bpy.data.actions['S'].layers[0].strips[0].channelbags[0].fcurves[0].update()
