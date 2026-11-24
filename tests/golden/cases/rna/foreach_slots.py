# M8 stabilization round 9: `foreach_get` / `foreach_set` on hook-served and ID-pointer
# collections, and `Node.mute` on nodes that cannot be muted. `Object.vertex_groups` (served by
# a hook) is written item by item. Empty slots of `mesh.materials` are skipped and take no
# buffer values (`[Material, None]` takes 1 roughness, `[Material, None, M2]` 2; the extra
# items of a longer `foreach_get` list are uninitialised memory in Blender, D-173, so their
# values are not compared). Output, group and zone nodes ignore `mute` writes. Values are moved
# into object locations so that they are compared.
# golden: startup=factory tier=T0 compare=objects,materials
import bpy
bpy.data.objects['Cube'].vertex_groups.new(name='G')
bpy.data.objects['Cube'].vertex_groups.new(name='H')
bpy.data.objects['Cube'].vertex_groups.foreach_set('lock_weight', [True, False])
bpy.data.objects['Cube'].vertex_groups.foreach_get('lock_weight', [False, False])
bpy.data.objects['Cube'].location.x = bpy.data.objects['Cube'].vertex_groups['G'].lock_weight
bpy.data.objects['Cube'].location.y = bpy.data.objects['Cube'].vertex_groups['H'].lock_weight
bpy.data.objects['Cube'].vertex_groups.foreach_set('lock_weight', [False])
bpy.data.meshes['Cube'].materials.append(None)
bpy.data.meshes['Cube'].materials.foreach_get('roughness', [0.0] * 2)
bpy.data.meshes['Cube'].materials.foreach_get('roughness', [7.0] * 3)
bpy.data.meshes['Cube'].materials.foreach_get('roughness', [0.0] * 1)
bpy.data.meshes['Cube'].materials.foreach_set('roughness', [0.2] * 2)
bpy.data.objects['Camera'].location.x = bpy.data.materials['Material'].roughness
bpy.data.materials.new('M2')
bpy.data.meshes['Cube'].materials.append(bpy.data.materials['M2'])
bpy.data.meshes['Cube'].materials.foreach_get('roughness', [0.0] * 1)
bpy.data.meshes['Cube'].materials.foreach_set('roughness', [0.3])
bpy.data.meshes['Cube'].materials.foreach_set('roughness', [0.3, 0.4])
bpy.data.meshes['Cube'].materials.foreach_set('roughness', [0.5, 0.6, 0.7])
bpy.data.objects['Camera'].location.y = bpy.data.materials['Material'].roughness
bpy.data.objects['Camera'].location.z = bpy.data.materials['M2'].roughness
bpy.data.meshes['Cube'].materials.foreach_get('foo', [0.0] * 2)
bpy.data.materials['Material'].node_tree.nodes['Material Output'].mute = True
bpy.data.objects['Light'].location.x = bpy.data.materials['Material'].node_tree.nodes['Material Output'].mute
bpy.data.materials['Material'].node_tree.nodes.foreach_set('mute', [True, True])
bpy.data.materials['M2'].use_nodes = True
bpy.data.materials['M2'].node_tree.nodes['Principled BSDF'].mute = True
bpy.data.materials['M2'].node_tree.nodes.new('NodeGroupInput').mute = True
bpy.data.materials['M2'].node_tree.nodes.new('ShaderNodeOutputAOV').mute = True
bpy.data.materials['M2'].node_tree.nodes.new('NodeReroute').mute = True
