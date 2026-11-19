# WP 4.4: SUBSURF on Suzanne at level 1: triangles, open eye holes (boundaries) and the nose
# vertex two quads share along two edges (valence 2, which Blender's Gregory patches place with
# the n/(n+5) rule at the isolation level).
# Positions agree within 1e-6; normals of the smallest faces amplify that to ~1e-5 (D-77).
# golden: startup=empty compare=evaluated:Suzanne tier=T1 tol=1e-4
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.data.objects['Suzanne'].modifiers.new(name='Subdivision', type='SUBSURF')
