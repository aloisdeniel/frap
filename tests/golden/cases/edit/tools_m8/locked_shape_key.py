# WP 8.3.3: the deform tools refuse a mesh whose active shape key is locked
# (`shape_key_report_if_locked`): face_make_planar, vertices_smooth_laplacian and
# symmetry_snap raise "The active shape key of Cube is locked"; unlocked, they run.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T2 tol=1e-5 order=exact
import bpy
bpy.ops.object.shape_key_add(from_mix=False)
bpy.ops.object.shape_key_add(from_mix=False)
bpy.context.object.active_shape_key.lock_shape = True
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.face_make_planar()
bpy.ops.mesh.vertices_smooth_laplacian()
bpy.ops.mesh.symmetry_snap()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.object.active_shape_key.lock_shape = False
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.vertices_smooth_laplacian(lambda_factor=0.2)
bpy.ops.mesh.symmetry_snap()
bpy.ops.mesh.face_make_planar()
