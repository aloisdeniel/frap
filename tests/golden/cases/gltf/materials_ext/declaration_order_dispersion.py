# M3+M4 stabilization round 5 (M34S5-2): where KHR_materials_dispersion is declared. The
# add-on's `export_dispersion` creates a (possibly empty) dispersion object for every material
# with a volume whose glTF Material Output group has a Dispersion socket, between the
# iridescence and IOR objects (`__gather_extensions`), so a volume material met before a sheen
# material declares dispersion before sheen even when a later material is the only one that
# keeps it: [transmission, volume, dispersion, sheen]. With the sheen material first, its empty
# transmission object comes first: [transmission, sheen, volume, dispersion]. The same order
# survives an import of the file (the importer's own glTF Material Output group) and a
# re-export.
# golden: startup=factory tier=T2 compare=glb:out/volume_first.glb,glb:out/sheen_first.glb,glb:out/reimported.glb glb_structure
import bpy
bpy.data.node_groups.new('glTF Material Output', 'ShaderNodeTree')
bpy.data.node_groups['glTF Material Output'].interface.new_socket('Occlusion', socket_type='NodeSocketFloat')
bpy.data.node_groups['glTF Material Output'].interface.new_socket('Thickness', socket_type='NodeSocketFloat')
bpy.data.node_groups['glTF Material Output'].interface.items_tree['Thickness'].default_value = 0.0
bpy.data.node_groups['glTF Material Output'].interface.new_socket('Dispersion', socket_type='NodeSocketFloat')
bpy.data.node_groups['glTF Material Output'].interface.items_tree['Dispersion'].default_value = 0.0
bpy.data.node_groups['glTF Material Output'].nodes.new('NodeGroupOutput')
bpy.data.node_groups['glTF Material Output'].nodes.new('NodeGroupInput')
bpy.data.materials.new('Glass')
bpy.data.materials['Glass'].node_tree.nodes['Principled BSDF'].inputs['Transmission Weight'].default_value = 1.0
bpy.data.materials['Glass'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Glass'].node_tree.nodes['Group'].node_tree = bpy.data.node_groups['glTF Material Output']
bpy.data.materials['Glass'].node_tree.nodes['Group'].inputs['Thickness'].default_value = 0.5
bpy.data.materials.new('Velvet')
bpy.data.materials['Velvet'].node_tree.nodes['Principled BSDF'].inputs['Sheen Weight'].default_value = 0.7
bpy.data.materials.new('Prism')
bpy.data.materials['Prism'].node_tree.nodes['Principled BSDF'].inputs['Transmission Weight'].default_value = 1.0
bpy.data.materials['Prism'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Prism'].node_tree.nodes['Group'].node_tree = bpy.data.node_groups['glTF Material Output']
bpy.data.materials['Prism'].node_tree.nodes['Group'].inputs['Thickness'].default_value = 1.0
bpy.data.materials['Prism'].node_tree.nodes['Group'].inputs['Dispersion'].default_value = 0.3
bpy.data.objects.remove(bpy.data.objects['Cube'])
bpy.ops.mesh.primitive_plane_add(location=(0, 0, 0))
bpy.context.object.name = 'A'
bpy.context.object.data.materials.append(bpy.data.materials['Glass'])
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.context.object.name = 'B'
bpy.context.object.data.materials.append(bpy.data.materials['Velvet'])
bpy.ops.mesh.primitive_plane_add(location=(6, 0, 0))
bpy.context.object.name = 'C'
bpy.context.object.data.materials.append(bpy.data.materials['Prism'])
bpy.ops.export_scene.gltf(filepath='out/volume_first.glb')
bpy.data.objects['A'].name = 'Z'
bpy.ops.export_scene.gltf(filepath='out/sheen_first.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/volume_first.glb')
bpy.ops.export_scene.gltf(filepath='out/reimported.glb')
