# WP 2.11: an L-shaped prism (reflex corners): outer miter PATCH and inner miter ARC.
# golden: startup=factory compare=objects,context,mesh:L tier=T2 order=exact
import bpy
bpy.data.meshes.new('L').from_pydata([(0, 0, 0), (2, 0, 0), (2, 1, 0), (1, 1, 0), (1, 2, 0), (0, 2, 0), (0, 0, 1), (2, 0, 1), (2, 1, 1), (1, 1, 1), (1, 2, 1), (0, 2, 1)], [], [(5, 4, 3, 2, 1, 0), (6, 7, 8, 9, 10, 11), (0, 1, 7, 6), (1, 2, 8, 7), (2, 3, 9, 8), (3, 4, 10, 9), (4, 5, 11, 10), (5, 0, 6, 11)])
bpy.data.objects['Cube'].data = bpy.data.meshes['L']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bevel(offset=0.2, segments=2, miter_outer='PATCH', miter_inner='ARC', spread=0.05)
