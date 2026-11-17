# WP 1.26: SyntaxError (code SYNTAX) with CPython's message, one outcome per source line. A
# line that does not parse is reported on its own and the lines around it still run; the
# unclosed bracket is the last line because both tools join an open bracket with the lines that
# follow it. The state is compared to show that nothing was written.
# golden: startup=factory tier=T0 compare=objects
import bpy
bpy.data.objects['Cube'].location = (1, 2, 3)
bpy.data.objects['Cube'].location =
bpy.ops.mesh.primitive_cube_add(size=2 location=(0, 0, 1))
bpy..data
bpy.data.objects['Cube'].location = (1, 2, 3))
bpy.data.objects['Cube']]
bpy.data.objects['Cube'].location = 1, 2,, 3
bpy.ops.mesh.primitive_cube_add(size=2,,)
bpy.data.objects['Cube'].location.x = 5
bpy.data.objects['Light'].location = (0, 0, 9
