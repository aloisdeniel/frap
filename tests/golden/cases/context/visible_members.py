# M1 stabilization: `visible_objects`, `selectable_objects`, `editable_objects` in base order,
# after hiding one object and making another unselectable; read through indices.
# golden: startup=factory tier=T0 compare=objects
import bpy
bpy.context.visible_objects
bpy.context.selectable_objects
bpy.context.editable_objects
bpy.data.objects['Cube'].hide_set(True)
bpy.data.objects['Light'].hide_select = True
bpy.data.objects['Camera'].name = bpy.context.visible_objects[0].name + '_v'
bpy.data.objects['Light'].name = bpy.context.selectable_objects[0].name + '_s'
bpy.data.objects['Cube'].name = bpy.context.editable_objects[1].name + '_e'
bpy.context.selectable_objects[1]
