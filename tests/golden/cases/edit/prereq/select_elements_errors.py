# WP 2.2: frap.select_elements errors: outside Edit Mode (poll), an index out of range, a
# negative index, an unknown object, an object that is not in Edit Mode, a bad enum value.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.ops.frap.select_elements(type='VERT', indices=[0])
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[8])
bpy.ops.frap.select_elements(type='FACE', indices=[1, 6])
bpy.ops.frap.select_elements(type='EDGE', indices=[-1])
bpy.ops.frap.select_elements(type='VERT', indices=[0], object='Nope')
bpy.ops.frap.select_elements(type='VERT', indices=[0], object='Camera')
bpy.ops.frap.select_elements(type='CORNER', indices=[0])
bpy.ops.frap.select_elements(type='VERT', indices=[0], action='TOGGLE')
bpy.ops.frap.select_elements(type='VERT', indices=[7], object='Cube')
