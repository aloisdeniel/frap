# WP 8.3.5 (T1), object.align: negative sides, centers and positive sides relative to the
# scene origin, the 3D cursor, the selection and the active object, high-quality mesh bounds
# and bound_box corners (objects without bounds count as their origin); nothing selected
# warns and cancels.
# golden: startup=factory compare=objects,context tier=T1 tol=1e-5
import bpy
bpy.data.objects['Cube'].rotation_euler = (0.3, 0.4, 0.5)
bpy.data.objects['Cube'].scale = (1, 2, 0.5)
bpy.ops.mesh.primitive_uv_sphere_add(radius=0.7, location=(3, -2, 1), rotation=(0.5, 0, 0.3))
bpy.data.objects['Cube'].select_set(True)
bpy.data.objects['Light'].select_set(True)
bpy.ops.object.align(align_axis={'X'})
bpy.ops.object.align(align_mode='OPT_1', relative_to='OPT_1', align_axis={'Y', 'Z'})
bpy.context.scene.cursor.location = (1, -1, 2)
bpy.ops.object.align(align_mode='OPT_3', relative_to='OPT_2', align_axis={'X', 'Z'})
bpy.ops.object.align(bb_quality=False, align_mode='OPT_2', relative_to='OPT_3', align_axis={'X', 'Y', 'Z'})
bpy.ops.object.align(align_mode='OPT_1', relative_to='OPT_4', align_axis={'Y'})
bpy.ops.object.align(bb_quality=False, align_mode='OPT_3', relative_to='OPT_4', align_axis={'Z'})
bpy.ops.object.align(relative_to='OPT_2')
bpy.ops.object.select_all(action='DESELECT')
bpy.ops.object.align(align_axis={'X'})
