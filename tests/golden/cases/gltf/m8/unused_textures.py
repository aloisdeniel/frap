# WP 8.12: `export_unused_textures` and `export_unused_images` (with `export_materials='EXPORT'`).
# Unused textures: every non-muted Image Texture node of an exported material (node groups
# included) that no exported texture reads is gathered as a texture from its Color output and
# appended to `textures`, `samplers` and `images` after everything else; the material reads its
# original node tree (no inlining). Unused images: every image of `bpy.data` that no texture
# wrote (not a Viewer) is appended to `images`. A texture used through Separate Color
# (metallic/roughness) is not gathered again; an Image Texture inside a node group is.
# golden: startup=factory tier=T2 compare=glb:out/tex.glb,glb:out/img.glb,glb:out/both.glb,glb:out/none.glb glb_structure
import bpy
bpy.data.images.new('Used', 4, 4)
bpy.data.images['Used'].generated_color = (0.9, 0.6, 0.3, 1.0)
bpy.data.images.new('Spare', 4, 4)
bpy.data.images['Spare'].generated_color = (0.1, 0.2, 0.3, 1.0)
bpy.data.images.new('Alone', 2, 2)
bpy.data.images['Alone'].generated_color = (0.5, 0.5, 0.5, 1.0)
bpy.data.images.new('ORM', 4, 4)
bpy.data.images['ORM'].generated_color = (1.0, 0.5, 0.25, 1.0)
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['Used']
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture.001'].image = bpy.data.images['Spare']
bpy.data.materials['Material'].node_tree.nodes['Image Texture.001'].interpolation = 'Closest'
bpy.data.materials['Material'].node_tree.nodes['Image Texture.001'].extension = 'EXTEND'
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture.002'].image = bpy.data.images['ORM']
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeSeparateColor')
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture.002'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Separate Color'].inputs['Color'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Separate Color'].outputs['Blue'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Metallic'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Separate Color'].outputs['Green'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Roughness'])
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture.003'].image = bpy.data.images['Spare']
bpy.data.materials['Material'].node_tree.nodes['Image Texture.003'].mute = True
bpy.data.images.new('InGroup', 3, 3)
bpy.data.node_groups.new('G', 'ShaderNodeTree')
bpy.data.node_groups['G'].nodes.new('ShaderNodeTexImage')
bpy.data.node_groups['G'].nodes['Image Texture'].image = bpy.data.images['InGroup']
bpy.data.node_groups['G'].nodes['Image Texture'].extension = 'MIRROR'
bpy.ops.mesh.primitive_plane_add(location=(3.0, 0.0, 0.0))
bpy.context.object.data.materials.append(bpy.data.materials.new('Grouped'))
bpy.data.materials['Grouped'].node_tree.nodes.new('ShaderNodeGroup')
bpy.data.materials['Grouped'].node_tree.nodes['Group'].node_tree = bpy.data.node_groups['G']
bpy.ops.export_scene.gltf(filepath='out/tex.glb', export_unused_textures=True)
bpy.ops.export_scene.gltf(filepath='out/img.glb', export_unused_images=True)
bpy.ops.export_scene.gltf(filepath='out/both.glb', export_unused_textures=True, export_unused_images=True)
bpy.ops.export_scene.gltf(filepath='out/none.glb', export_unused_images=True, export_unused_textures=True, export_materials='PLACEHOLDER')
