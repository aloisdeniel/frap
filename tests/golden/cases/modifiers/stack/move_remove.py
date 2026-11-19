# WP 4.1 (T0): obj.modifiers.move/remove/clear, modifiers.active, Modifier.is_active and
# use_pin_to_last (pinning moves the modifier), and the object.modifier_move_up/down/
# move_to_index/copy/set_active/remove/modifiers_clear operators with their refusals. The
# active modifier's name is encoded as a probe object's name, flags in its location.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects.new('Probe', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Probe'])
bpy.data.objects['Cube'].modifiers.new('A', 'SUBSURF')
bpy.data.objects['Cube'].modifiers.new('B', 'BEVEL')
bpy.data.objects['Cube'].modifiers.new('C', 'WELD')
bpy.data.objects['Cube'].modifiers.new('D', 'MIRROR')
bpy.data.objects['Cube'].modifiers.new('E', 'MULTIRES')
bpy.data.objects['Cube'].modifiers.move(0, 3)
bpy.data.objects['Cube'].modifiers.move(4, 1)
bpy.data.objects['Cube'].modifiers.move(99, 2)
bpy.data.objects['Cube'].modifiers.move(2, 99)
bpy.data.objects['Cube'].modifiers.move(2, -1)
bpy.data.objects['Cube'].modifiers.move(1, 0)
bpy.data.objects['Cube'].modifiers.move(0, 2)
bpy.data.objects['Cube'].modifiers.remove(bpy.data.objects['Cube'].modifiers['E'])
bpy.data.objects['Cube'].modifiers.remove(None)
bpy.data.objects['Cube'].modifiers.active = bpy.data.objects['Cube'].modifiers['B']
bpy.data.objects['Probe'].name = bpy.data.objects['Cube'].modifiers.active.name
bpy.data.objects['Cube'].modifiers.remove(bpy.data.objects['Cube'].modifiers['B'])
bpy.data.objects['B'].name = bpy.data.objects['Cube'].modifiers.active.name
bpy.data.objects['Cube'].modifiers.active = None
bpy.data.objects['Cube'].modifiers.active.name
bpy.data.objects['Cube'].modifiers['A'].is_active = True
bpy.data.objects['Cube'].modifiers['A'].is_active = False
bpy.data.objects[1].location.x = bpy.data.objects['Cube'].modifiers['A'].is_active
bpy.data.objects['Cube'].modifiers['A'].use_pin_to_last = True
bpy.data.objects['Cube'].modifiers['C'].use_pin_to_last = True
bpy.data.objects['Cube'].modifiers['D'].use_pin_to_last = False
bpy.ops.object.modifier_add(type='TRIANGULATE')
bpy.ops.object.modifier_move_down(modifier='Triangulate')
bpy.ops.object.modifier_move_up(modifier='A')
bpy.ops.object.modifier_move_up(modifier='D')
bpy.ops.object.modifier_move_up(modifier='Nope')
bpy.ops.object.modifier_move_down(modifier='A')
bpy.ops.object.modifier_move_to_index(modifier='Triangulate', index=0)
bpy.ops.object.modifier_move_to_index(modifier='Triangulate', index=9)
bpy.ops.object.modifier_move_to_index(modifier='Triangulate', index=3)
bpy.data.objects['Cube'].modifiers['C'].use_pin_to_last = False
bpy.ops.object.modifier_copy(modifier='D')
bpy.ops.object.modifier_copy(modifier='Nope')
bpy.ops.object.modifier_set_active(modifier='A')
bpy.data.objects[1].name = bpy.data.objects['Cube'].modifiers.active.name
bpy.ops.object.modifier_set_active(modifier='')
bpy.data.objects['Cube'].modifiers.active.name
bpy.ops.object.modifier_remove(modifier='D.001', report=True)
bpy.ops.object.modifier_remove(modifier='Nope')
bpy.ops.object.modifier_remove()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.object.modifier_remove(modifier='D')
bpy.ops.object.modifier_move_up(modifier='Triangulate')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Cube'].modifiers.new('X', 'SUBSURF')
bpy.data.objects['Camera'].modifiers.remove(bpy.data.objects['Cube'].modifiers['X'])
bpy.data.objects['Camera'].select_set(True)
bpy.ops.object.modifiers_clear()
bpy.data.objects['Cube'].modifiers.clear()
bpy.data.objects['Cube'].modifiers.clear()
bpy.context.view_layer.objects.active = bpy.data.objects['Camera']
bpy.ops.object.modifiers_clear()
bpy.ops.object.modifier_set_active(modifier='X')
bpy.ops.object.modifier_remove(modifier='X')
bpy.context.view_layer.objects.active = None
bpy.ops.object.modifiers_clear()
bpy.ops.object.modifier_remove(modifier='X')
bpy.ops.object.modifier_set_active(modifier='X')
