# WP 2.12: proportional editing in Edit Mode: every deterministic falloff (world distance to
# the nearest selected vertex), and the projected (view) distance.
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1 tol=1e-5
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=8, y_subdivisions=8, size=4, rotation=(0.3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[27, 28])
bpy.ops.transform.translate(value=(0, 0, 0.5), use_proportional_edit=True, proportional_size=1.5, proportional_edit_falloff='SMOOTH')
bpy.ops.transform.translate(value=(0, 0, 0.2), use_proportional_edit=True, proportional_size=1.2, proportional_edit_falloff='SPHERE')
bpy.ops.transform.translate(value=(0, 0.1, 0), use_proportional_edit=True, proportional_size=2, proportional_edit_falloff='ROOT')
bpy.ops.transform.translate(value=(0.1, 0, 0), use_proportional_edit=True, proportional_size=1, proportional_edit_falloff='INVERSE_SQUARE')
bpy.ops.transform.rotate(value=0.3, orient_axis='Z', use_proportional_edit=True, proportional_size=1.7, proportional_edit_falloff='SHARP')
bpy.ops.transform.resize(value=(1.3, 1.3, 1.3), use_proportional_edit=True, proportional_size=1.4, proportional_edit_falloff='LINEAR')
bpy.ops.transform.translate(value=(0, 0, -0.3), use_proportional_edit=True, proportional_size=0.9, proportional_edit_falloff='CONSTANT')
bpy.ops.transform.translate(value=(0, 0, 0.4), use_proportional_edit=True, proportional_size=1.6, use_proportional_projected=True)
