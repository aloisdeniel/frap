# WP 1.26: reads, writes and calls of attributes that do not exist (code RNA_NO_ATTRIBUTE,
# AttributeError) on IDs, nested structs, `bpy.data`, `bpy.context`, math values and
# collections, with the type name Blender prints.
# golden: startup=factory tier=T0 compare=objects
import bpy
bpy.data.objects['Cube'].foo
bpy.data.foo
bpy.context.foo
bpy.data.objects['Cube'].data.foo
bpy.data.objects['Cube'].location.foo
bpy.data.scenes['Scene'].render.foo
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].foo
bpy.data.objects['Cube'].foo = 1
bpy.data.objects['Cube'].Location = (1, 2, 3)
bpy.data.objects['Cube'].foo()
bpy.data.objects.foo()
bpy.data.objects['Cube'].location.foo()
bpy.context.object.foo
bpy.data.materials['Material'].node_tree.nodes['Material Output'].inputs['Surface'].default_value
