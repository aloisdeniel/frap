# WP 2.12: translate in Edit Mode moves the selected vertices by a world vector, whatever the
# object's location, rotation and scale; GLOBAL, LOCAL and a one-axis constraint.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1 tol=1e-5
import bpy
bpy.data.objects['Cube'].location = (1, 2, 0.5)
bpy.data.objects['Cube'].rotation_euler = (0.3, 0, 0.7)
bpy.data.objects['Cube'].scale = (2, 1, 0.5)

bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 1, 4])
bpy.ops.transform.translate(value=(1, 0.5, -0.25))
bpy.ops.transform.translate(value=(0.5, 0, 0), orient_type='LOCAL')
bpy.ops.transform.translate(value=(0, 0, 0.3), orient_type='LOCAL', constraint_axis=(False, False, True))
