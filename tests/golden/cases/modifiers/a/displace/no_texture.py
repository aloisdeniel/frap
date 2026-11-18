# WP 4.2: DISPLACE without a texture (white: strength · (1 − mid_level)): along the vertex
# normals (default), along local X, along global Z of a rotated object, and with
# `CUSTOM_NORMAL`; zero strength disables the modifier, RGB_TO_XYZ without a texture too.
# golden: startup=factory compare=evaluated:Cube,evaluated:Cube.001,evaluated:Cube.002,evaluated:Cube.003,evaluated:Cube.004 tier=T1
import bpy
bpy.ops.object.duplicate()
bpy.ops.object.duplicate()
bpy.ops.object.duplicate()
bpy.ops.object.duplicate()
bpy.data.objects['Cube'].modifiers.new(name='Displace', type='DISPLACE')
bpy.data.objects['Cube.001'].modifiers.new(name='Displace', type='DISPLACE')
bpy.data.objects['Cube.001'].modifiers['Displace'].direction = 'X'
bpy.data.objects['Cube.001'].modifiers['Displace'].strength = 2.5
bpy.data.objects['Cube.001'].modifiers['Displace'].mid_level = 0.2
bpy.data.objects['Cube.002'].rotation_euler = (0.4, 0.2, 0.9)
bpy.data.objects['Cube.002'].scale = (1, 2, 1)
bpy.data.objects['Cube.002'].modifiers.new(name='Displace', type='DISPLACE')
bpy.data.objects['Cube.002'].modifiers['Displace'].direction = 'Z'
bpy.data.objects['Cube.002'].modifiers['Displace'].space = 'GLOBAL'
bpy.data.objects['Cube.003'].modifiers.new(name='Displace', type='DISPLACE')
bpy.data.objects['Cube.003'].modifiers['Displace'].direction = 'CUSTOM_NORMAL'
bpy.data.objects['Cube.003'].modifiers['Displace'].strength = -0.3
bpy.data.objects['Cube.004'].modifiers.new(name='Displace', type='DISPLACE')
bpy.data.objects['Cube.004'].modifiers['Displace'].strength = 0
bpy.data.objects['Cube.004'].modifiers.new(name='Displace.001', type='DISPLACE')
bpy.data.objects['Cube.004'].modifiers['Displace.001'].direction = 'RGB_TO_XYZ'
