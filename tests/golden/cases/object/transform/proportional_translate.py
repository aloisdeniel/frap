# WP 1.21 (T0), proportional editing in Object Mode: unselected objects (children of the
# selection included) move by the falloff of their distance to the selection.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Light'].parent = bpy.data.objects['Cube']
bpy.data.objects['Light'].location = (1, 0, 0)
bpy.data.objects['Camera'].location = (1, 1, 1)
bpy.ops.transform.translate(value=(1, 0, 0), use_proportional_edit=True, proportional_size=3, proportional_edit_falloff='LINEAR')
