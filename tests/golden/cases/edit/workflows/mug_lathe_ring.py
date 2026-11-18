# WP 2.13 workflow (mug handle ring / lathe): a plane reduced to its outline (faces deleted),
# moved out from the axis and stood up in the XZ plane, then spun a full turn around Z in 16
# steps; the seam where the last step meets the first is merged by spin's auto merge.
# golden: startup=empty compare=objects,context,mesh:Plane tier=T2 tol=1e-4
import bpy
bpy.ops.mesh.primitive_plane_add(size=0.2)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.delete(type='ONLY_FACE')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.transform.rotate(value=1.5707963267948966, orient_axis='X')
bpy.ops.transform.translate(value=(0.6, 0, 0))
bpy.ops.mesh.spin(steps=16, angle=6.283185307179586, center=(0, 0, 0), axis=(0, 0, 1))
bpy.data.meshes['Plane'].total_vert_sel
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.normals_make_consistent(inside=False)
