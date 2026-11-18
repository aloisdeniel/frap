# WP 2.4: mesh.select_random equals Blender's for the same seed: BLI_rng shuffle of the
# visible elements of the select mode, int(count * ratio) of them, then flushed; DESELECT
# flushes from the vertices. Two objects in Edit Mode: the second adds the hash of its name.
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
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.mesh.select_random(ratio=0.6, seed=1234)
bpy.ops.mesh.select_random(ratio=0.5, seed=5, action='DESELECT')
