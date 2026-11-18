# WP 4.3: LAPLACIANSMOOTH with its defaults (λ 0.01, normalized, volume preserved) on Suzanne
# (open borders at the eyes).
# golden: startup=empty compare=evaluated:Suzanne tier=T1 tol=1e-4
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.data.objects['Suzanne'].modifiers.new(name='M', type='LAPLACIANSMOOTH')
