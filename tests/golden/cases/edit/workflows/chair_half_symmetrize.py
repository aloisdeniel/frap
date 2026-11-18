# WP 2.13 workflow (chair): model half a chair and mirror it. The seat's bottom is cut into a
# 3x3 grid, the two corner cells on +X are extruded down as legs, the -X half is cut away
# with mesh.bisect, and mesh.symmetrize rebuilds it from the +X side.
# golden: startup=empty compare=objects,context,mesh:Cube tier=T2
import bpy
bpy.ops.mesh.primitive_cube_add(size=1, location=(0, 0, 0.45), scale=(1, 1, 0.1))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[4])
bpy.ops.mesh.subdivide(number_cuts=2)
bpy.ops.frap.select_elements(type='FACE', indices=[4, 12])
bpy.ops.mesh.extrude_region_move(TRANSFORM_OT_translate={"value": (0, 0, -0.4)})
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bisect(plane_co=(0, 0, 0), plane_no=(1, 0, 0), clear_inner=True)
bpy.data.meshes['Cube'].total_vert_sel
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.symmetrize(direction='POSITIVE_X')
