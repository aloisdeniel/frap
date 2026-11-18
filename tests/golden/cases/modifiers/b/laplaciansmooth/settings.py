# WP 4.3: LAPLACIANSMOOTH: strong λ over 3 iterations on a closed sphere; area-weighted
# (not normalized) with a negative border λ and no volume preservation on a grid.
# golden: startup=empty compare=evaluated:Sphere,evaluated:Grid tier=T1 tol=1e-4
import bpy
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=8)
bpy.data.objects['Sphere'].modifiers.new(name='M', type='LAPLACIANSMOOTH')
bpy.data.objects['Sphere'].modifiers['M'].lambda_factor = 2.0
bpy.data.objects['Sphere'].modifiers['M'].iterations = 3
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=5, size=2, location=(3, 0, 0))
bpy.data.objects['Grid'].modifiers.new(name='Wave', type='SIMPLE_DEFORM')
bpy.data.objects['Grid'].modifiers['Wave'].deform_method = 'BEND'
bpy.data.objects['Grid'].modifiers.new(name='M', type='LAPLACIANSMOOTH')
bpy.data.objects['Grid'].modifiers['M'].lambda_factor = 1.5
bpy.data.objects['Grid'].modifiers['M'].lambda_border = -0.5
bpy.data.objects['Grid'].modifiers['M'].use_normalized = False
bpy.data.objects['Grid'].modifiers['M'].use_volume_preserve = False
bpy.data.objects['Grid'].modifiers['M'].iterations = 2
bpy.data.objects['Grid'].modifiers['M'].use_z = False
