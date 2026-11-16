# Seed case of the golden harness (WP 1.8): an operator, a read, and the scene it leaves.
# golden: startup=factory compare=objects,mesh:Cube.001,context tier=T1 tol=1e-5
import bpy
bpy.ops.mesh.primitive_cube_add(size=2, location=(0, 0, 1))
bpy.data.objects['Cube.001'].location
