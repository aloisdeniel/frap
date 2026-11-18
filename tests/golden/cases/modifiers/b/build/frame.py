# WP 4.3: BUILD follows the scene frame: at frame 31 (after the update) 30 % of Suzanne's
# faces are built.
# golden: startup=empty compare=evaluated:Suzanne tier=T1
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.data.objects['Suzanne'].modifiers.new(name='M', type='BUILD')
bpy.context.scene.frame_current = 31
bpy.context.view_layer.update()
