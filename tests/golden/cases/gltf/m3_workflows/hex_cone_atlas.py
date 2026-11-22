# WP 3.9: uv.smart_project (angle limit 66 degrees, margin 0.01, as in uv_pack_atlas.py) on a
# six-sided cone. The six equal-area sides tie in smart_project's area sort, which Blender runs
# through the C library qsort: macOS libc sorts up to 7 elements by insertion (and takes
# FreeBSD's "nevermind" insertion shortcut after a partition without swaps), so the order of
# the tied sides decides the projection groups. frap ports that qsort (`frap_uv::pack::
# qsort_libc`, WP 8.1; the BSD port partitioned from 7 elements on and 11 UV corners differed):
# T2 against Blender.
# golden: startup=factory tier=T2 tol=1e-5 compare=mesh:Cone
import bpy
bpy.data.objects.remove(bpy.data.objects['Cube'])
bpy.ops.mesh.primitive_cone_add(vertices=6)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.smart_project(angle_limit=1.1519173, island_margin=0.01)
bpy.ops.object.mode_set(mode='OBJECT')
