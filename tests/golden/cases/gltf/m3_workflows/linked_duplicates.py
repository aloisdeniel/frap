# WP 3.9: a forest of linked duplicates. object.duplicate_move_linked makes two more trees
# sharing the first tree's mesh (one glTF mesh referenced by three nodes); a fourth tree is a
# full copy (object.duplicate_move: its own mesh). The second tree overrides its material with
# an OBJECT-linked slot, so its node needs its own glTF mesh with another material. Round trip
# (§17.2): import shares the mesh between the nodes again.
# golden: startup=factory tier=T2 compare=glb:out/forest.glb,glb:out/forest_rt.glb
import bpy
bpy.data.objects.remove(bpy.data.objects['Cube'])
bpy.ops.mesh.primitive_cone_add(vertices=8, radius1=1, depth=2, location=(0, 0, 1))
bpy.data.objects['Cone'].name = 'Tree'
bpy.data.materials['Material'].name = 'Leaves'
bpy.data.materials['Leaves'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.1, 0.5, 0.1, 1.0)
bpy.data.objects['Tree'].data.materials.append(bpy.data.materials['Leaves'])
bpy.ops.object.duplicate_move_linked(TRANSFORM_OT_translate={'value': (3, 0, 0)})
bpy.ops.object.duplicate_move_linked(TRANSFORM_OT_translate={'value': (0, 3, 0)})
bpy.ops.object.duplicate_move(TRANSFORM_OT_translate={'value': (-6, 0, 0)})
bpy.data.materials.new('Autumn')
bpy.data.materials['Autumn'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.8, 0.4, 0.05, 1.0)
bpy.data.objects['Tree.001'].material_slots[0].link = 'OBJECT'
bpy.data.objects['Tree.001'].material_slots[0].material = bpy.data.materials['Autumn']
bpy.ops.export_scene.gltf(filepath='out/forest.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/forest.glb')
bpy.ops.export_scene.gltf(filepath='out/forest_rt.glb')
