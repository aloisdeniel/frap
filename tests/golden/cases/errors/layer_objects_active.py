# M1 stabilization: assignment errors on a property of a collection's API struct use the
# `BPy_PropertyRNA - Attribute (setattr):` prefix (`view_layer.objects.active`).
# golden: startup=factory tier=T0 compare=context
import bpy
bpy.context.view_layer.objects.active = 'Cube'
bpy.context.view_layer.objects.active = 5
bpy.context.view_layer.objects.active = bpy.data.meshes['Cube']
bpy.context.view_layer.objects.active = bpy.data.objects['Light']
