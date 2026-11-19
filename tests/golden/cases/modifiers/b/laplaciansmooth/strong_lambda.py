# WP 4.3: LAPLACIANSMOOTH with an extreme λ (1000) over 4 iterations on Suzanne: an
# ill-conditioned system. frap's solver is not Eigen's SparseLU, so corner normals of the faces
# this shrinks to near-degenerate size differ by up to ~3e-4 (listed deviation); positions agree
# to 1e-4.
# golden: startup=empty compare=evaluated:Suzanne tier=T1 tol=5e-4
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.data.objects['Suzanne'].modifiers.new(name='M', type='LAPLACIANSMOOTH')
bpy.data.objects['Suzanne'].modifiers['M'].lambda_factor = 1000.0
bpy.data.objects['Suzanne'].modifiers['M'].iterations = 4
