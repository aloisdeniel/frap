# WP 2.12: nothing selected: the transform is cancelled (also with proportional editing);
# hidden vertices never move.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1 tol=1e-5
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.transform.translate(value=(1, 0, 0))
bpy.ops.transform.translate(value=(1, 0, 0), use_proportional_edit=True)
bpy.ops.transform.rotate(value=0.3)
bpy.ops.transform.resize()
