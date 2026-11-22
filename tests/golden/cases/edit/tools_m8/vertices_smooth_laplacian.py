# WP 8.3.3: mesh.vertices_smooth_laplacian (implicit cotangent Laplacian, a sparse
# least-squares solve: T2): the whole monkey with defaults and with repeat, lambda and an axis
# off, volume preservation off; part of a subdivided grid (the border vertices stay boundary,
# the unselected neighbours are locked at zero); nothing selected: WARNING and CANCELLED.
# golden: startup=factory compare=objects,context,mesh:Suzanne,mesh:Grid tier=T2 tol=1e-5 order=exact
import bpy
bpy.ops.object.delete()
bpy.ops.mesh.primitive_monkey_add(enter_editmode=True)
bpy.ops.mesh.vertices_smooth_laplacian()
bpy.ops.mesh.vertices_smooth_laplacian(repeat=3, lambda_factor=0.4, lambda_border=0.2, use_y=False)
bpy.ops.mesh.vertices_smooth_laplacian(lambda_factor=2.0, preserve_volume=False)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=8, y_subdivisions=8, size=2, location=(4, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.transform.vertex_random(offset=0.2, seed=3)
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[18, 19, 20, 26, 27, 28, 34, 35, 36, 42])
bpy.ops.mesh.vertices_smooth_laplacian(repeat=2, lambda_factor=1.5, lambda_border=0.5)
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.vertices_smooth_laplacian(lambda_factor=0.8, lambda_border=0.3, preserve_volume=False)
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.vertices_smooth_laplacian()
