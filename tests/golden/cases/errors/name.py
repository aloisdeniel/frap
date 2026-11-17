# WP 1.26: NameError (code NAME) for a lone identifier and for an unknown function call, with
# Python's message; the statements after them still run.
# golden: startup=factory tier=T0 compare=objects
import bpy
cube
Cube
cube_add()
primitive_cube_add(size=2)
bpy_data
bpy.data.objects['Cube'].location = (0, 1, 0)
