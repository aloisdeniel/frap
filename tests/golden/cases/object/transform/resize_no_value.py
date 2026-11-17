# WP 1.21 (T0), transform.resize without value: Blender's unset value is (0, 0, 0), every object
# collapses to the pivot; a constraint keeps the other axes.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Light'].select_set(True)
bpy.data.objects['Cube'].scale = (2, 3, 4)
bpy.ops.transform.resize()
bpy.data.objects['Light'].select_set(False)
bpy.data.objects['Camera'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Camera']
bpy.ops.transform.resize(constraint_axis=(True, False, False))
