# WP 2.11: five segments (vertex mesh subdivided to 8, then interpolated) with a
# non-circular profile (general superellipse chords).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bevel(offset=0.25, segments=5, profile=0.7)
