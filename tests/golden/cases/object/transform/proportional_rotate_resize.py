# WP 1.21 (T0), proportional rotate and resize in Object Mode (SHARP and SMOOTH falloffs).
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Camera'].location = (1, 1, 1)
bpy.ops.transform.rotate(value=0.5, use_proportional_edit=True, proportional_size=3, proportional_edit_falloff='SHARP', center_override=(0, 0, 0))
bpy.ops.transform.resize(value=(2, 2, 2), use_proportional_edit=True, proportional_size=3, proportional_edit_falloff='SMOOTH')
