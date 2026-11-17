# WP 1.26: path forms of [RNA §1.1], read on the right-hand side of a write so that the value
# each one resolves to lands in the compared state: string keys, positive and negative
# indices, the `(name, None)` tuple key, `get`/`find`, nested structs, nameless mesh items,
# attribute data, UV data, material slots, node sockets by name and by index, links, matrix
# elements and rows, vector components and `path_resolve`. Names are moved into object names
# with the same trick.
# golden: startup=factory tier=T0 compare=objects
import bpy
bpy.data.objects['Cube'].location = bpy.data.meshes['Cube'].vertices[0].co
bpy.data.objects['Cube'].scale = bpy.data.objects[-1].data.color
bpy.data.objects['Cube'].rotation_euler = bpy.data.objects[0].rotation_euler
bpy.data.objects['Light'].location = bpy.data.objects['Cube', None].data.vertices[-1].co
bpy.data.objects['Light'].scale = bpy.data.meshes['Cube'].attributes['position'].data[6].vector
bpy.data.objects['Light'].location.z = bpy.data.objects.get('Camera').matrix_world[0][3]
bpy.data.objects['Light'].location.y = bpy.data.objects.find('Light')
bpy.data.objects['Light'].scale.x = bpy.data.meshes['Cube'].uv_layers['UVMap'].data[3].uv[0]
bpy.data.objects['Light'].scale.y = bpy.data.meshes['Cube'].polygons[0].vertices[2]
bpy.data.objects['Light'].scale.z = bpy.data.meshes['Cube'].edges[3].vertices[1]
bpy.data.objects['Camera'].scale = bpy.data.objects['Cube'].active_material.node_tree.nodes['Principled BSDF'].inputs[0].default_value[0:3]
bpy.data.objects['Camera'].location = bpy.data.objects['Camera'].matrix_world[1][0:3]
bpy.data.objects['Camera'].location.x = bpy.data.scenes[0].view_layers['ViewLayer'].objects.active.location.z
bpy.data.objects['Camera'].rotation_euler.x = bpy.data.collections['Collection'].all_objects['Light'].data.energy
bpy.data.objects['Camera'].rotation_euler.y = bpy.data.cameras['Camera'].lens
bpy.data.objects['Camera'].rotation_euler.z = bpy.data.objects['Camera'].path_resolve('location[2]')
bpy.data.objects['Camera'].location.y = bpy.data.objects['Camera'].location[-1]
bpy.data.objects['Camera'].name = bpy.data.materials['Material'].node_tree.links[0].to_socket.name
bpy.data.objects['Light'].name = bpy.data.materials['Material'].node_tree.nodes[0].name
bpy.data.objects['Cube'].name = bpy.data.scenes['Scene'].collection.children[0].objects[1].name
bpy.data.objects['Surface'].name = bpy.data.objects.get('Nope', bpy.data.objects['Material Output']).name
