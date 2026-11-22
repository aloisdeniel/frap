# WP 8.3.5 (T1), Object Mode transforms with a selected child of a selected parent and
# proportional editing: Blender deselects the child for the transform
# (set_trans_object_base_flags), then proportional editing takes it as an unselected object,
# so its location moves by its falloff factor on top of following its parent (shear,
# translate and rotate).
# golden: startup=factory compare=objects,context tier=T1 tol=1e-5
import bpy
bpy.data.objects['Cube'].parent = bpy.data.objects['Light']
bpy.data.objects['Light'].select_set(True)
bpy.ops.transform.shear(angle=0.4, use_proportional_edit=True, proportional_size=20)
bpy.ops.transform.translate(value=(0.3, 0.2, 0.1), use_proportional_edit=True, proportional_edit_falloff='LINEAR', proportional_size=20)
bpy.ops.transform.rotate(value=0.3, orient_axis='Y', use_proportional_edit=True, proportional_size=15)
