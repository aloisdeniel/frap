# Seed case of the golden harness (WP 1.8): Blender's exception classes and messages (T0).
# golden: startup=factory tier=T0
import bpy
bpy.ops.mesh.primitive_cube_add(sizee=2)
bpy.ops.mesh.primitive_cube_add(size="x")
bpy.ops.mesh.primitive_cube_add(align='FOO')
bpy.ops.mesh.foo()
bpy.data.objects['Nope']
bpy.data.objects['Cube'].location = (1, 2)
