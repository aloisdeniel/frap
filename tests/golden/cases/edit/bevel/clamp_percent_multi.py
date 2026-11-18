# WP 2.11: two objects in Edit Mode beveled by one call (each mesh with its own selection),
# with clamp_overlap and the PERCENT offset type (a collision limit of 50 percent).
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cube.001 tier=T2 order=exact
import bpy
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0), scale=(2, 1, 1))
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Cube'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bevel(offset_type='PERCENT', offset_pct=80, segments=2, clamp_overlap=True)
