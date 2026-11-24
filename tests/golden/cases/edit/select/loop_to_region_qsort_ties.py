# M8 stabilization round 5: mesh.loop_to_region visits the selected edges in the order of the
# macOS C library qsort (verg_radial: face count, most first). Every edge of an icosphere has
# two faces, so all the keys tie and the unstable qsort order decides which regions are found
# first: frap sorted stably and selected 44 faces where Blender 5.2.4 selects 45 (face 0).
# golden: startup=factory compare=objects,context,mesh:Icosphere tier=T1
import bpy
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=3)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_random(ratio=0.2, seed=11)
bpy.ops.mesh.select_more()
bpy.ops.mesh.select_less()
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.mesh.loop_to_region()
