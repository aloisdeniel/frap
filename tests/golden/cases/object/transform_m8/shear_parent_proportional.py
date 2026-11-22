# WP 8.3.5 (T1), transform.shear in Object Mode on a parented child (its parent-space
# location is sheared about the world pivot), with a delta location, and with proportional
# editing (unselected objects move by their falloff factor).
# golden: startup=factory compare=objects,context tier=T1 tol=1e-5
import bpy
bpy.data.objects['Light'].parent = bpy.data.objects['Cube']
bpy.data.objects['Cube'].location = (1, 0.5, 0)
bpy.data.objects['Cube'].rotation_euler = (0, 0, 0.6)
bpy.data.objects['Cube'].select_set(False)
bpy.data.objects['Light'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Light']
bpy.ops.transform.shear(angle=0.5)
bpy.data.objects['Camera'].delta_location = (0.5, 1, -1)
bpy.data.objects['Light'].select_set(False)
bpy.data.objects['Camera'].select_set(True)
bpy.ops.transform.shear(angle=0.3, orient_axis='X', orient_axis_ortho='Y')
bpy.data.objects['Camera'].select_set(False)
bpy.data.objects['Cube'].select_set(True)
bpy.ops.transform.shear(angle=0.4, use_proportional_edit=True, proportional_edit_falloff='LINEAR', proportional_size=12)
bpy.ops.transform.shear(angle=0.2, use_proportional_edit=True, proportional_edit_falloff='SMOOTH', proportional_size=9)
