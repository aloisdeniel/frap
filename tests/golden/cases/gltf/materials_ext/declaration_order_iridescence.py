# M5+M6 W0 (M3+M4 leftover): where KHR_materials_iridescence is declared. The add-on's
# `export_iridescence` creates a (possibly empty) iridescence object for every material with a
# Principled BSDF and a glTF Material Output group with the `Iridescence Factor` and
# `Iridescence Thickness Minimum` sockets, so a grouped material met before a sheen material
# declares iridescence before sheen even when a later material is the only one that keeps it:
# [iridescence, sheen]. With the sheen material first: [sheen, iridescence]. The same order
# survives an import of the file (the importer's own glTF Material Output group) and a
# re-export.
# golden: startup=factory tier=T2 compare=glb:out/grouped_first.glb,glb:out/sheen_first.glb,glb:out/reimported.glb glb_structure
import bpy
bpy.data.node_groups.new('glTF Material Output', 'ShaderNodeTree')
bpy.data.node_groups['glTF Material Output'].interface.new_socket('Occlusion', socket_type='NodeSocketFloat')
bpy.data.node_groups['glTF Material Output'].interface.new_socket('Iridescence Factor', socket_type='NodeSocketFloat')
bpy.data.node_groups['glTF Material Output'].interface.items_tree['Iridescence Factor'].default_value = 0.0
bpy.data.node_groups['glTF Material Output'].interface.new_socket('Iridescence Thickness Minimum', socket_type='NodeSocketFloat')
bpy.data.node_groups['glTF Material Output'].interface.items_tree['Iridescence Thickness Minimum'].default_value = 100.0
bpy.data.node_groups['glTF Material Output'].nodes.new('NodeGroupOutput')
bpy.data.node_groups['glTF Material Output'].nodes.new('NodeGroupInput')
bpy.data.materials.new('Grouped')
bpy.data.materials['Grouped'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Grouped'].node_tree.nodes['Group'].node_tree = bpy.data.node_groups['glTF Material Output']
bpy.data.materials.new('Velvet')
bpy.data.materials['Velvet'].node_tree.nodes['Principled BSDF'].inputs['Sheen Weight'].default_value = 0.7
bpy.data.materials.new('Pearl')
bpy.data.materials['Pearl'].node_tree.nodes['Principled BSDF'].inputs['Thin Film Thickness'].default_value = 500.0
bpy.data.materials['Pearl'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Pearl'].node_tree.nodes['Group'].node_tree = bpy.data.node_groups['glTF Material Output']
bpy.data.materials['Pearl'].node_tree.nodes['Group'].inputs['Iridescence Factor'].default_value = 1.0
bpy.data.objects.remove(bpy.data.objects['Cube'])
bpy.ops.mesh.primitive_plane_add(location=(0, 0, 0))
bpy.context.object.name = 'A'
bpy.context.object.data.materials.append(bpy.data.materials['Grouped'])
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.context.object.name = 'B'
bpy.context.object.data.materials.append(bpy.data.materials['Velvet'])
bpy.ops.mesh.primitive_plane_add(location=(6, 0, 0))
bpy.context.object.name = 'C'
bpy.context.object.data.materials.append(bpy.data.materials['Pearl'])
bpy.ops.export_scene.gltf(filepath='out/grouped_first.glb')
bpy.data.objects['A'].name = 'Z'
bpy.ops.export_scene.gltf(filepath='out/sheen_first.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/grouped_first.glb')
bpy.ops.export_scene.gltf(filepath='out/reimported.glb')
