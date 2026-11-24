# M8 stabilization round 7: `foreach_get` on collections of IDs served item by item
# (`scene.objects`, `collection.objects` / `all_objects`, `view_layer.objects`) reads like
# `foreach_set` already wrote (it raised `internal error setting the array`); a longer buffer
# is accepted, a shorter one raises. Enum properties go through as their item numbers
# (`material_slots` 'link', `objects` 'type' / 'rotation_mode'); a float for an enum raises
# TypeError, a string property raises the size error. Blender writes the buffer before it
# raises that TypeError, the float as -1 (D-173), so the line repeats the current values.
# golden: startup=factory tier=T0 compare=objects
import bpy
bpy.data.scenes['Scene'].objects.foreach_get('location', [0.0] * 9)
bpy.data.scenes['Scene'].objects.foreach_get('location', [0.0] * 8)
bpy.data.collections['Collection'].objects.foreach_get('location', [0.0] * 9)
bpy.data.collections['Collection'].all_objects.foreach_get('location', [0.0] * 10)
bpy.context.view_layer.objects.foreach_get('location', [0.0] * 9)
bpy.data.scenes['Scene'].objects.foreach_get('dimensions', [0.0] * 9)
bpy.data.scenes['Scene'].objects.foreach_get('dimensions', [0.0] * 3)
bpy.data.scenes['Scene'].objects.foreach_get('hide_render', [False] * 3)
bpy.data.objects['Cube'].material_slots.foreach_get('link', [''])
bpy.data.objects['Cube'].material_slots.foreach_get('slot_index', [0])
bpy.data.objects.foreach_get('type', [0] * 3)
bpy.data.scenes['Scene'].objects.foreach_get('type', [0] * 3)
bpy.data.objects.foreach_set('rotation_mode', [0, -1, 1])
bpy.data.objects.foreach_get('rotation_mode', [0] * 3)
bpy.data.objects.foreach_set('rotation_mode', [0, 1.5, 1])
bpy.data.objects.foreach_get('name', [0] * 3)
bpy.data.objects.foreach_set('name', ['a', 'b', 'c'])
