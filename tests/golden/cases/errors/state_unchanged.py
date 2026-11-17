# WP 1.26: an error leaves the state as it was (§13.1) and the next statements run against that
# state: a failed operator adds nothing (no object, no mesh, no name taken: the next cube is
# still `Cube.001`), a failed write keeps every component, a failed function call selects or
# links nothing.
# golden: startup=factory tier=T0 compare=objects,materials,context
import bpy
bpy.data.objects['Cube'].location = (1, 2, 3)
bpy.ops.mesh.primitive_cube_add(size='x', location=(5, 5, 5))
bpy.ops.mesh.primitive_cube_add(location=(5, 5))
bpy.ops.mesh.primitive_cube_add(sizee=1, location=(5, 5, 5))
bpy.data.objects['Cube'].location = (9, 9, 'x')
bpy.data.objects['Cube'].location = (9, 9)
bpy.data.objects['Cube'].rotation_mode = 'FOO'
bpy.data.objects['Camera'].select_set('yes')
bpy.context.view_layer.objects.active = bpy.data.objects['Nope']
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (1, 0, 0)
bpy.ops.mesh.primitive_cube_add(size=1, location=(0, 0, 4))
bpy.data.objects['Cube.001'].name
bpy.data.meshes['Cube.001'].name
