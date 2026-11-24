# M8 stabilization round 10: `mesh.materials[i] = m` / `= None` writes the slot (Blender's
# errors for a value that is not a Material or None, a string key, an index out of range, and
# "failed assignment" on collections without an assign callback); `Mesh.validate()` repairs
# duplicate edges, duplicate faces and non-finite positions and answers True; a `mute` write on
# a node that cannot be muted is ignored. No `foreach_get` runs while the first slot is empty
# (it crashes Blender, D-176). Values are moved into object locations so that they are compared.
# golden: startup=factory tier=T0 compare=objects,materials,mesh:Cube
import bpy
bpy.data.materials.new('M2')
bpy.data.materials['M2'].roughness = 0.25
bpy.data.meshes['Cube'].materials.append(bpy.data.materials['Material'])
bpy.data.meshes['Cube'].materials[0] = bpy.data.materials['M2']
bpy.data.meshes['Cube'].materials[1] = None
bpy.data.meshes['Cube'].materials.foreach_get('roughness', [0.0] * 1)
bpy.data.objects['Camera'].location.x = bpy.data.objects['Cube'].material_slots[0].material.roughness
bpy.data.meshes['Cube'].materials[-1] = bpy.data.materials['Material']
bpy.data.meshes['Cube'].materials.foreach_set('roughness', [0.75, 0.125])
bpy.data.objects['Camera'].location.y = bpy.data.materials['Material'].roughness
bpy.data.meshes['Cube'].materials[2] = None
bpy.data.meshes['Cube'].materials[-3] = None
bpy.data.meshes['Cube'].materials[0] = 5
bpy.data.meshes['Cube'].materials[0] = bpy.data.objects['Cube']
bpy.data.meshes['Cube'].materials[0] = bpy.data.materials['M2'].node_tree
bpy.data.meshes['Cube'].materials['M2'] = None
bpy.data.meshes['Cube'].materials[0:1] = None
bpy.data.objects[0] = None
bpy.data.objects[9] = None
bpy.data.materials[0] = 5
bpy.data.meshes['Cube'].vertices[0] = bpy.data.meshes['Cube'].vertices[1]
bpy.data.meshes['Cube'].materials[-2] = None
bpy.data.objects['Light'].location.x = bpy.data.objects['Cube'].active_material_index
bpy.data.meshes['Cube'].validate()
bpy.data.meshes['Cube'].edges.add(1)
bpy.data.meshes['Cube'].edges[12].vertices = (5, 7)
bpy.data.objects['Light'].location.y = bpy.data.meshes['Cube'].validate()
bpy.data.meshes['Cube'].vertices[0].co = (float('nan'), 1, 2)
bpy.data.objects['Light'].location.z = bpy.data.meshes['Cube'].validate()
bpy.data.meshes['Cube'].attributes.new('fw', 'FLOAT', 'FACE')
bpy.data.meshes['Cube'].loops.add(4)
bpy.data.meshes['Cube'].loops.foreach_set('vertex_index', [0, 4, 6, 2, 3, 2, 6, 7, 7, 6, 4, 5, 5, 1, 3, 7, 1, 0, 2, 3, 5, 4, 0, 1, 0, 4, 6, 2])
bpy.data.meshes['Cube'].loops.foreach_set('edge_index', [10, 9, 6, 7, 4, 6, 3, 8, 3, 9, 5, 0, 1, 11, 8, 0, 2, 7, 4, 11, 5, 10, 2, 1, 10, 9, 6, 7])
bpy.data.meshes['Cube'].polygons.add(1)
bpy.data.meshes['Cube'].polygons[6].loop_start = 24
bpy.data.meshes['Cube'].attributes['fw'].data.foreach_set('value', [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0])
bpy.data.objects['Cube'].location.x = bpy.data.meshes['Cube'].validate()
bpy.data.meshes['Cube'].validate()
bpy.data.materials['Material'].node_tree.nodes['Material Output'].mute = True
bpy.data.objects['Cube'].location.y = bpy.data.materials['Material'].node_tree.nodes['Material Output'].mute
