# WP 1.26: an operator that does not exist (code OP_UNKNOWN, AttributeError): unknown name in a
# known namespace, unknown namespace, near-miss spelling, and `.poll()` of an unknown operator
# (Blender says "Polling operator" there). Nothing is added.
# golden: startup=factory tier=T0 compare=objects,context
import bpy
bpy.ops.mesh.foo()
bpy.ops.foo.bar()
bpy.ops.mesh.primitive_cube_ad()
bpy.ops.mesh.primitive_cube_add_()
bpy.ops.object.selectall(action='SELECT')
bpy.ops.mesh.foo(size=2)
bpy.ops.mesh.foo.poll()
bpy.ops.foo.bar.poll()
