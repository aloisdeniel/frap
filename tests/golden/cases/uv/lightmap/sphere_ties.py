# M3+M4 stabilization round 2: uv.lightmap_pack on a 12x6 UV sphere. Many faces have boxes
# of equal size, and the box packer sorts them with the C library `qsort`, whose order of
# equal keys is the platform's (deviations.md §2, qsort row): frap's BSD port places some
# faces in other slots than macOS libc does. The UV value sets agree but corners land on
# other boxes, so this compares at T3 (every UV in [0, 1], boxes disjoint, topology kept).
# golden: startup=factory compare=objects,mesh:Sphere tier=T3 check=uv01,uv_no_overlap,faces:72-72,euler:2
import bpy
bpy.ops.object.delete()
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.lightmap_pack()
bpy.ops.object.mode_set(mode='OBJECT')
