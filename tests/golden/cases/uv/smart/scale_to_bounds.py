# WP 3.8: uv.smart_project on a stretched cube with only some faces selected and
# scale_to_bounds: the selected faces' islands are packed, then stretched to fill [0, 1]².
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.transform.resize(value=(2, 1, 0.5))
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1, 4])
bpy.ops.uv.smart_project(scale_to_bounds=True, margin_method='ADD', island_margin=0.02)
bpy.ops.object.mode_set(mode='OBJECT')
