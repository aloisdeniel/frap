# WP 1.26: API removed before 5.x (code RNA_REMOVED_IN_5X): frap answers with Blender 5.2's
# AttributeError (and a hint that Blender does not give); reads, writes and calls alike.
# golden: startup=factory tier=T0 compare=objects
import bpy
bpy.data.meshes['Cube'].use_auto_smooth
bpy.data.meshes['Cube'].use_auto_smooth = True
bpy.data.meshes['Cube'].auto_smooth_angle
bpy.data.meshes['Cube'].calc_normals()
bpy.data.objects['Cube'].data.use_auto_smooth
