# WP 3.8: uv.pack_islands(shape_method='AABB') and 'CONVEX' with the default rotate_method
# ANY on a cube-projected Suzanne. The pre-rotated boxes of mirror-image islands are equal to
# the last bit, so their order follows float rounding (Blender's arm64 FMA contraction and
# platform sinf/cosf) and two mirrored islands swap slots against Blender: T3 invariants
# (uv01 only: a cube projection of Suzanne overlaps itself inside islands).
# golden: startup=empty compare=objects,context,mesh:Suzanne tier=T3 check=uv01
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.cube_project()
bpy.ops.uv.pack_islands(shape_method='AABB')
bpy.ops.uv.pack_islands(shape_method='CONVEX', margin=0.01)
bpy.ops.object.mode_set(mode='OBJECT')
