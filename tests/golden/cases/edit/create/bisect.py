# WP 2.8: mesh.bisect cuts the selection with a world-space plane: a cube cut through its
# middle (the cut is selected), a tilted cut on a moved and rotated object filled and with
# the inner side cleared, a cut through existing vertices (vertices on the plane are reused),
# and CANCELLED without a selection.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cube.001 tier=T2
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bisect(plane_co=(0, 0, 0.25), plane_no=(0, 0, 1))
bpy.data.meshes['Cube'].total_vert_sel
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bisect(plane_co=(0, 0, 0), plane_no=(1, 0, 0))
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.bisect(plane_co=(0, 0, 0), plane_no=(0, 1, 0))
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.select_all(action='DESELECT')
bpy.ops.mesh.primitive_cube_add(size=2, location=(4, 0, 0), rotation=(0.3, 0.2, 0.1))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bisect(plane_co=(4, 0, 0.2), plane_no=(0.2, 0.3, 1), use_fill=True, clear_inner=True)
bpy.data.meshes['Cube.001'].total_face_sel
