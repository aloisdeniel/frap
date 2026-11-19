# WP 3.6: KHR_materials_specular export. specularFactor = 2 x Specular IOR Level when below 1
# (first material); above 1 the excess multiplies specularColorFactor (second material, tint
# times 1.6); a specular colour texture (copied whole) on the third.
# golden: startup=factory tier=T2 compare=glb:out/specular.glb
import bpy
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Specular IOR Level'].default_value = 0.3
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Specular Tint'].default_value = (0.5, 0.6, 0.7, 1)
bpy.data.materials.new('Shiny')
bpy.data.materials['Shiny'].node_tree.nodes['Principled BSDF'].inputs['Specular IOR Level'].default_value = 0.8
bpy.data.materials['Shiny'].node_tree.nodes['Principled BSDF'].inputs['Specular Tint'].default_value = (0.25, 0.5, 0.75, 1)
bpy.data.materials.new('Tinted')
bpy.data.images.new('Tint', 4, 4)
bpy.data.materials['Tinted'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Tinted'].node_tree.nodes['Image Texture'].image = bpy.data.images['Tint']
bpy.data.materials['Tinted'].node_tree.links.new(bpy.data.materials['Tinted'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Tinted'].node_tree.nodes['Principled BSDF'].inputs['Specular Tint'])
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.context.object.data.materials.append(bpy.data.materials['Shiny'])
bpy.ops.mesh.primitive_plane_add(location=(-3, 0, 0))
bpy.context.object.data.materials.append(bpy.data.materials['Tinted'])
bpy.ops.export_scene.gltf(filepath='out/specular.glb')
