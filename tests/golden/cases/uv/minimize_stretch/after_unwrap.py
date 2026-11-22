# WP 8.1: uv.minimize_stretch(iterations=10) after a CONFORMAL unwrap of Suzanne cut along her
# default seams. The binary searches compare stretches that are nearly equal near the optimum,
# so the unwrap's own differences from Blender (up to 1.5e-7, LSCM solved with another sparse
# solver) grow to 4.6e-5 after 10 iterations: tol 1e-4 (from identical UVs the stretch
# minimisation is bit-identical, suzanne.py).
# golden: startup=empty compare=objects,context,mesh:Suzanne tier=T2 tol=1e-4
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.seams_from_islands()
bpy.ops.uv.unwrap(method='CONFORMAL')
bpy.ops.uv.minimize_stretch(iterations=10)
bpy.ops.object.mode_set(mode='OBJECT')
