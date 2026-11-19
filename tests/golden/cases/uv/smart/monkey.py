# WP 3.8: uv.smart_project on Suzanne (many islands, triangles and quads, packed with
# AXIS_ALIGNED_Y): T3 invariant (UVs in the unit square; the concave head folds within some
# projection groups, so islands may overlap themselves, in Blender too).
# golden: startup=empty compare=objects,context,mesh:Suzanne tier=T3 check=uv01
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.smart_project(angle_limit=1.0, island_margin=0.01)
bpy.ops.object.mode_set(mode='OBJECT')
