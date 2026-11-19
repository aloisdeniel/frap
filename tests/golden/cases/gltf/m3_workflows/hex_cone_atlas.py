# WP 3.9: uv.smart_project (angle limit 66 degrees, margin 0.01, as in uv_pack_atlas.py) on a
# six-sided cone. The six equal-area sides tie in smart_project's area sort, which Blender runs
# through the C library qsort: macOS libc sorts 7 elements by insertion, while frap's port of
# the BSD qsort (frap-uv) partitions from 7 elements on, so the sides fall into other
# projection groups and 11 UV corners differ from Blender's (a known gap, WP 3.3/3.8
# follow-up; the 8-sided cone of uv_pack_atlas.py matches exactly). T3 invariants until then:
# UVs in the unit square, islands disjoint.
# golden: startup=factory tier=T3 compare=mesh:Cone check=uv01,uv_no_overlap
import bpy
bpy.data.objects.remove(bpy.data.objects['Cube'])
bpy.ops.mesh.primitive_cone_add(vertices=6)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.smart_project(angle_limit=1.1519173, island_margin=0.01)
bpy.ops.object.mode_set(mode='OBJECT')
