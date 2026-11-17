# WP 1.19 (T0), object.select_all: INVERT flips the visible bases only; an object hidden with
# hide_set stays unselected (and keeps that state when shown again).
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Light'].hide_set(True)
bpy.ops.object.select_all(action='INVERT')
bpy.data.objects['Light'].hide_set(False)
