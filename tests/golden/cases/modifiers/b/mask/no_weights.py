# WP 4.3: MASK on a mesh without vertex-group weights: empty (VERTEX_GROUP and ARMATURE
# modes), unchanged when inverted.
# golden: startup=factory compare=evaluated:Cube,evaluated:Cube.001,evaluated:Cube.002 tier=T1
import bpy
bpy.ops.object.duplicate()
bpy.ops.object.duplicate()
bpy.data.objects['Cube'].modifiers.new(name='M', type='MASK')
bpy.data.objects['Cube.001'].modifiers.new(name='M', type='MASK')
bpy.data.objects['Cube.001'].modifiers['M'].invert_vertex_group = True
bpy.data.objects['Cube.002'].modifiers.new(name='M', type='MASK')
bpy.data.objects['Cube.002'].modifiers['M'].mode = 'ARMATURE'
