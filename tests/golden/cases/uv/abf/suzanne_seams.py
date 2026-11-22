# WP 8.1: ABF++ on Suzanne (more than 100 faces per chart: the gradient limit is 1.0), cut
# along the seams of her default UV map, narrowed first so that no island is wider than tall:
# the packer turns a wide island by +90 or -90 degrees on the sign of a residual angle of
# about 1e-7, a knife edge that differences far below the tolerance can flip (see
# suzanne_default.py). The islands are averaged and packed as Blender's.
# golden: startup=empty compare=objects,context,mesh:Suzanne tier=T2 tol=1e-5
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.transform.resize(value=(0.6, 1.0, 1.0))
bpy.ops.uv.seams_from_islands()
bpy.ops.uv.unwrap(method='ANGLE_BASED')
bpy.ops.object.mode_set(mode='OBJECT')
