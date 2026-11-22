# WP 8.12: `KHR_materials_anisotropy` baked from grayscale textures
# (`export_anisotropy_from_grayscale_textures`): images behind `Anisotropic` and `Anisotropic
# Rotation` outside the importer's setup make a new RGB image `Image`: B the strength's grey
# value, R and G the unit direction of the rotation's grey value × 2π mapped to [0, 1]. The
# strength factor is 1 when the socket has no factor (a Multiply by 0.5 gives 0.5); a second
# material has only a rotation texture (B white), a third only a strength texture (an
# unlinked rotation counts as 1: R = 1, G = 0.5).
# golden: startup=factory tier=T2 compare=glb:out/aniso.glb glb_structure
import bpy
bpy.data.images.new('Strength', 2, 2)
bpy.data.images['Strength'].pixels = [0.2, 0.2, 0.2, 1.0, 0.4, 0.5, 0.6, 1.0, 0.8, 0.8, 0.8, 1.0, 1.0, 1.0, 1.0, 1.0]
bpy.data.images.new('Angle', 2, 2)
bpy.data.images['Angle'].pixels = [0.0, 0.0, 0.0, 1.0, 0.125, 0.125, 0.125, 1.0, 0.3, 0.6, 0.1, 1.0, 0.75, 0.75, 0.75, 1.0]
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['Strength']
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture.001'].image = bpy.data.images['Angle']
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Anisotropic'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture.001'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Anisotropic Rotation'])
bpy.data.materials.new('RotOnly')
bpy.data.materials['RotOnly'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['RotOnly'].node_tree.nodes['Image Texture'].image = bpy.data.images['Angle']
bpy.data.materials['RotOnly'].node_tree.links.new(bpy.data.materials['RotOnly'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['RotOnly'].node_tree.nodes['Principled BSDF'].inputs['Anisotropic Rotation'])
bpy.data.materials['RotOnly'].node_tree.nodes['Principled BSDF'].inputs['Anisotropic'].default_value = 0.7
bpy.data.materials.new('StrengthOnly')
bpy.data.materials['StrengthOnly'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['StrengthOnly'].node_tree.nodes['Image Texture'].image = bpy.data.images['Strength']
bpy.data.materials['StrengthOnly'].node_tree.nodes.new('ShaderNodeMath')
bpy.data.materials['StrengthOnly'].node_tree.nodes['Math'].operation = 'MULTIPLY'
bpy.data.materials['StrengthOnly'].node_tree.nodes['Math'].inputs[1].default_value = 0.5
bpy.data.materials['StrengthOnly'].node_tree.links.new(bpy.data.materials['StrengthOnly'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['StrengthOnly'].node_tree.nodes['Math'].inputs[0])
bpy.data.materials['StrengthOnly'].node_tree.links.new(bpy.data.materials['StrengthOnly'].node_tree.nodes['Math'].outputs[0], bpy.data.materials['StrengthOnly'].node_tree.nodes['Principled BSDF'].inputs['Anisotropic'])
bpy.data.materials['StrengthOnly'].node_tree.nodes['Principled BSDF'].inputs['Anisotropic Rotation'].default_value = 0.25
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.context.object.data.materials.append(bpy.data.materials['RotOnly'])
bpy.ops.mesh.primitive_plane_add(location=(-3, 0, 0))
bpy.context.object.data.materials.append(bpy.data.materials['StrengthOnly'])
bpy.ops.export_scene.gltf(filepath='out/aniso.glb')
