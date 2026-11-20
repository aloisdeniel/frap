# WP 6.1: AnimData and action / slot assignment (animrig::generic_assign_action and
# generic_assign_action_slot): animation_data_create() defaults, slot auto-assignment by the
# ID's name, by last_slot_identifier and to a lone untyped slot (bound to the ID type on
# assignment, `XXThing` → `OBThing`), the remembered identifier after un-assigning, slot
# errors, slot renames propagating to their users, animation_data_clear(). Values are moved into
# object transforms and object names so they are compared.
# golden: startup=factory tier=T1 tol=1e-5 compare=anim:A,anim:B,objects
import bpy
bpy.data.actions.new('A')
bpy.data.actions['A'].slots.new(id_type='OBJECT', name='Cube')
bpy.data.actions.new('B')
bpy.data.actions['B'].slots.new(id_type='OBJECT', name='Other')
bpy.data.actions.new('C')
bpy.data.actions['C'].slots.new(id_type='UNSPECIFIED', name='Thing')
bpy.data.objects['Cube'].animation_data_create()
bpy.data.objects['Camera'].location = (bpy.data.objects['Cube'].animation_data.action_influence, bpy.data.objects['Cube'].animation_data.use_nla, bpy.data.objects['Cube'].animation_data.action_slot_handle)
bpy.data.objects.new(bpy.data.objects['Cube'].animation_data.action_blend_type + bpy.data.objects['Cube'].animation_data.action_extrapolation, None)
bpy.data.objects['Cube'].animation_data.action_slot = bpy.data.actions['A'].slots[0]
bpy.data.objects['Cube'].animation_data.action = bpy.data.actions['A']
bpy.data.objects['Cube'].location = (bpy.data.objects['Cube'].animation_data.action_slot_handle - 929201140, bpy.data.actions['A'].users, 0)
bpy.data.objects.new('last_' + bpy.data.objects['Cube'].animation_data.last_slot_identifier, None)
bpy.data.objects['Cube'].animation_data.action = bpy.data.actions['B']
bpy.data.objects['Cube'].scale = (bpy.data.objects['Cube'].animation_data.action_slot_handle, bpy.data.actions['A'].users + 1, bpy.data.actions['B'].users + 1)
bpy.data.objects['Cube'].animation_data.action_slot = bpy.data.actions['A'].slots[0]
bpy.data.objects['Cube'].animation_data.action_slot = bpy.data.actions['B'].slots[0]
bpy.data.objects.new('slot_' + bpy.data.objects['Cube'].animation_data.action_slot.identifier, None)
bpy.data.objects['Cube'].animation_data.action_slot = None
bpy.data.objects.new('kept_' + bpy.data.objects['Cube'].animation_data.last_slot_identifier, None)
bpy.data.actions['B'].slots.new(id_type='MATERIAL', name='Mat')
bpy.data.objects['Cube'].animation_data.action_slot = bpy.data.actions['B'].slots['MAMat']
bpy.data.objects['Cube'].animation_data.action_slot_handle = 929201142
bpy.data.actions['B'].slots['OBOther'].name_display = 'Renamed'
bpy.data.objects.new('renamed_' + bpy.data.objects['Cube'].animation_data.last_slot_identifier, None)
bpy.data.actions['B'].slots['OBRenamed'].identifier = 'XXIdentified'
bpy.data.objects.new('identified_' + bpy.data.objects['Cube'].animation_data.last_slot_identifier, None)
bpy.data.actions['B'].slots[0].name_display = ''
bpy.data.actions['B'].slots[0].identifier = 'OB'
bpy.data.objects['Camera'].animation_data_create()
bpy.data.objects['Camera'].animation_data.action = bpy.data.actions['C']
bpy.data.objects.new('lone_' + bpy.data.objects['Camera'].animation_data.action_slot.identifier, None)
bpy.data.objects['Camera'].animation_data.action_slot.target_id_type = 'MATERIAL'
bpy.data.objects['Light'].animation_data_create()
bpy.data.objects['Light'].animation_data.action = bpy.data.actions['C']
bpy.data.objects['Light'].location = (bpy.data.objects['Light'].animation_data.action_slot_handle, bpy.data.actions['C'].users, 0)
bpy.data.objects['Light'].animation_data.action_influence = 0.25
bpy.data.objects['Light'].animation_data.action_blend_type = 'ADD'
bpy.data.objects['Light'].animation_data.action_extrapolation = 'NOTHING'
bpy.data.objects['Light'].animation_data.use_nla = False
bpy.data.objects['Light'].scale = (bpy.data.objects['Light'].animation_data.action_influence, bpy.data.objects['Light'].animation_data.use_nla, 1)
bpy.data.objects.new(bpy.data.objects['Light'].animation_data.action_blend_type + bpy.data.objects['Light'].animation_data.action_extrapolation, None)
bpy.data.actions['C'].slots.new(id_type='OBJECT', name='Second')
bpy.data.objects.new('suitable_' + bpy.data.objects['Light'].animation_data.action_suitable_slots[1].identifier, None)
bpy.data.materials['Material'].animation_data_create()
bpy.data.materials['Material'].animation_data.action = bpy.data.actions['B']
bpy.data.objects.new('material_' + bpy.data.materials['Material'].animation_data.action_slot.identifier, None)
bpy.data.objects['Cube'].animation_data_clear()
bpy.data.objects['Cube'].animation_data.action
bpy.data.actions.remove(bpy.data.actions['C'])
bpy.data.objects['Light'].rotation_euler = (bpy.data.objects['Light'].animation_data.action_slot_handle, bpy.data.objects['Camera'].animation_data.action_slot_handle, 0)
