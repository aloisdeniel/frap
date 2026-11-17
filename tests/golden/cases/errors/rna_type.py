# WP 1.26: values of the wrong type (code RNA_TYPE, TypeError): float arrays, single
# components, strings, booleans (0/1 accepted, 2 and 1.0 refused), ints, pointers of the wrong
# ID type or to themselves, a string for a pointer, an enum given an int, and a vector indexed
# by a string. Failed writes change nothing; the accepted ones are compared.
# golden: startup=factory tier=T0 compare=objects
import bpy
bpy.data.objects['Cube'].location = 'abc'
bpy.data.objects['Cube'].location = (5, 'a', 0)
bpy.data.objects['Cube'].location = None
bpy.data.objects['Cube'].location.x = 'a'
bpy.data.objects['Cube'].location[0] = 'a'
bpy.data.objects['Cube'].matrix_world = 1
bpy.data.objects['Cube'].name = 1
bpy.data.objects['Cube'].name = None
bpy.data.objects['Cube'].hide_render = 'x'
bpy.data.objects['Cube'].hide_render = 2
bpy.data.objects['Cube'].hide_render = 1.0
bpy.data.objects['Cube'].hide_render = None
bpy.data.objects['Cube'].hide_render = 1
bpy.data.objects['Cube'].parent = bpy.data.meshes['Cube']
bpy.data.objects['Cube'].parent = 'Camera'
bpy.data.objects['Cube'].parent = bpy.data.objects['Cube']
bpy.data.objects['Cube'].pass_index = 1.5
bpy.data.objects['Cube'].pass_index = 'a'
bpy.data.objects['Cube'].rotation_mode = 1
bpy.data.scenes['Scene'].frame_start = 5.5
bpy.data.scenes['Scene'].camera = bpy.data.meshes['Cube']
bpy.data.scenes['Scene'].render.filepath = 5
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 'a'
bpy.data.objects['Cube'].location['x']
bpy.data.objects['Cube'].scale = (True, 2, 3)
