# M8 stabilization round 5: mesh.loop_to_region(select_bigger=True) with every face of the cube
# selected in face mode: every edge is selected and the regions tie, so the macOS qsort order
# of the edges (all with two faces) decides the half that is kept. frap sorted stably and kept
# the other half (found by the differential fuzzer, WP 8.7).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.loop_to_region(select_bigger=True)
