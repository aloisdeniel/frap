# WP 2.4: edit/select/random.py cut after its line 14,
#   bpy.ops.mesh.select_random(ratio=0.3, seed=11)
# so that call's effect reaches the dump (later lines of the full case replace it).
# golden: startup=factory compare=objects,context,mesh:Grid,mesh:Cube tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=8, y_subdivisions=8, location=(4, 0, 0))
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_random(seed=3)
bpy.ops.mesh.select_random(ratio=0.25, seed=7, action='DESELECT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_random(ratio=0.3, seed=11)
