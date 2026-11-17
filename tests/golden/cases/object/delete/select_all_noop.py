# WP 1.19 (T0), object.select_all: an action that changes nothing is CANCELLED (SELECT with
# everything selected, DESELECT with nothing selected); the active object stays unselected.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.select_all(action='DESELECT')
bpy.ops.object.select_all(action='DESELECT')
