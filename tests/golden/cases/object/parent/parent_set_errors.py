# WP 1.21 (T0), object.parent_set errors: a parent loop, BONE without an armature, FOLLOW without
# a curve (CANCELLED), no active object (poll), only the active object selected (no-op).
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.ops.object.parent_set()
bpy.data.objects['Cube'].parent = bpy.data.objects['Light']
bpy.data.objects['Light'].select_set(True)
bpy.ops.object.parent_set()
bpy.data.objects['Cube'].parent = None
bpy.ops.object.parent_set(type='BONE')
bpy.ops.object.parent_set(type='FOLLOW')
bpy.context.view_layer.objects.active = None
bpy.ops.object.parent_set()
