# WP 4.1 (T0): object.modifier_add and obj.modifiers.new: insertion positions (HOOK after the
# leading deform-only run, before pinned modifiers), unique names, single-instance types,
# unsupported objects and types, the active modifier and the default show flags. Values are
# encoded in the location of probe empties so that the objects dump compares them.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.ops.object.modifier_add(type='SUBSURF')
bpy.ops.object.modifier_add(type='HOOK')
bpy.ops.object.modifier_add(type='MIRROR')
bpy.ops.object.modifier_add(type='CLOTH')
bpy.ops.object.modifier_add(type='CLOTH')
bpy.ops.object.modifier_add(type='GREASE_PENCIL_TINT')
bpy.ops.object.modifier_add(type='SURFACE')
bpy.ops.object.modifier_add(type='NOPE')
bpy.data.objects['Cube'].modifiers.new('', 'BEVEL')
bpy.data.objects['Cube'].modifiers.new('Subdivision', 'BEVEL')
bpy.data.objects['Cube'].modifiers.new('X', 'CLOTH')
bpy.data.objects['Cube'].modifiers.new('X', 'GREASE_PENCIL_TINT')
bpy.data.objects['Cube'].modifiers.new('X', 'NOPE')
bpy.data.objects['Cube'].modifiers.new(name='X')
bpy.data.objects['Camera'].modifiers.new('X', 'SUBSURF')
bpy.data.objects['Cube'].modifiers.new('A' * 70, 'WELD')
bpy.data.objects.new('Probe', None)
bpy.data.objects['Probe'].name = bpy.data.objects['Cube'].modifiers.active.name
bpy.data.objects.new('Flags', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Flags'])
bpy.data.objects['Flags'].location.x = bpy.data.objects['Cube'].modifiers['Subdivision'].show_in_editmode
bpy.data.objects['Flags'].location.y = bpy.data.objects['Cube'].modifiers['Hook'].show_in_editmode
bpy.data.objects['Flags'].location.z = bpy.data.objects['Cube'].modifiers['Hook'].show_on_cage
bpy.data.objects['Flags'].scale.x = bpy.data.objects['Cube'].modifiers['Hook'].show_expanded
bpy.data.objects['Flags'].scale.y = bpy.data.objects['Cube'].modifiers['Hook'].show_render
bpy.data.objects['Flags'].scale.z = bpy.data.objects['Cube'].modifiers['Subdivision'].is_active
bpy.data.objects['Cube'].modifiers.new('Pinned', 'TRIANGULATE')
bpy.data.objects['Cube'].modifiers['Pinned'].use_pin_to_last = True
bpy.ops.object.modifier_add(type='WELD')
bpy.ops.object.modifier_add(type='MULTIRES')
bpy.data.objects.new('Empty', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Empty'])
bpy.context.view_layer.objects.active = bpy.data.objects['Empty']
bpy.ops.object.modifier_add(type='SUBSURF')
bpy.ops.object.modifier_add(type='NODES')
bpy.context.view_layer.objects.active = bpy.data.objects['Light']
bpy.ops.object.modifier_add(type='SUBSURF')
bpy.ops.object.modifier_add.poll()
bpy.context.view_layer.objects.active = None
bpy.ops.object.modifier_add(type='SUBSURF')
bpy.data.objects['Cube'].select_set(True)
bpy.data.objects['Empty'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Empty']
bpy.ops.object.modifier_add(type='NODES', use_selected_objects=True)
