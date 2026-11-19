# WP 3.9: car paint. A metallic red body with KHR_materials_clearcoat driven by one image of
# distinct pixels: its R channel (Separate Color) is the Coat Weight (clearcoatTexture) and its G
# channel the Coat Roughness (clearcoatRoughnessTexture), the add-on's channel sharing; a coat
# normal map (Strength 0.6) gives clearcoatNormalTexture. A chrome trim has a constant coat of
# 1 with Blender's default coat roughness. Round trip (§17.2).
# golden: startup=factory tier=T2 compare=glb:out/car.glb,glb:out/car_rt.glb
import bpy
bpy.data.objects['Cube'].name = 'Body'
bpy.data.objects['Body'].scale = (2.0, 1.0, 0.5)
bpy.data.materials['Material'].name = 'Paint'
bpy.data.materials['Paint'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.7, 0.02, 0.02, 1.0)
bpy.data.materials['Paint'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 0.6
bpy.data.materials['Paint'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.35
bpy.data.images.new('CoatMask', 2, 2)
bpy.data.images['CoatMask'].colorspace_settings.name = 'Non-Color'
bpy.data.images['CoatMask'].pixels = [1.0, 0.05, 0.0, 1.0, 0.8, 0.2, 0.0, 1.0, 0.6, 0.4, 0.0, 1.0, 0.3, 0.9, 0.0, 1.0]
bpy.data.images.new('Flakes', 2, 2)
bpy.data.images['Flakes'].colorspace_settings.name = 'Non-Color'
bpy.data.images['Flakes'].pixels = [0.5, 0.5, 1.0, 1.0, 0.6, 0.45, 0.95, 1.0, 0.4, 0.55, 0.95, 1.0, 0.5, 0.4, 0.9, 1.0]
bpy.data.materials['Paint'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Paint'].node_tree.nodes['Image Texture'].image = bpy.data.images['CoatMask']
bpy.data.materials['Paint'].node_tree.nodes.new('ShaderNodeSeparateColor')
bpy.data.materials['Paint'].node_tree.links.new(bpy.data.materials['Paint'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Paint'].node_tree.nodes['Separate Color'].inputs['Color'])
bpy.data.materials['Paint'].node_tree.links.new(bpy.data.materials['Paint'].node_tree.nodes['Separate Color'].outputs['Red'], bpy.data.materials['Paint'].node_tree.nodes['Principled BSDF'].inputs['Coat Weight'])
bpy.data.materials['Paint'].node_tree.links.new(bpy.data.materials['Paint'].node_tree.nodes['Separate Color'].outputs['Green'], bpy.data.materials['Paint'].node_tree.nodes['Principled BSDF'].inputs['Coat Roughness'])
bpy.data.materials['Paint'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Paint'].node_tree.nodes['Image Texture.001'].image = bpy.data.images['Flakes']
bpy.data.materials['Paint'].node_tree.nodes.new('ShaderNodeNormalMap')
bpy.data.materials['Paint'].node_tree.nodes['Normal Map'].inputs['Strength'].default_value = 0.6
bpy.data.materials['Paint'].node_tree.links.new(bpy.data.materials['Paint'].node_tree.nodes['Image Texture.001'].outputs['Color'], bpy.data.materials['Paint'].node_tree.nodes['Normal Map'].inputs['Color'])
bpy.data.materials['Paint'].node_tree.links.new(bpy.data.materials['Paint'].node_tree.nodes['Normal Map'].outputs['Normal'], bpy.data.materials['Paint'].node_tree.nodes['Principled BSDF'].inputs['Coat Normal'])
bpy.ops.mesh.primitive_cylinder_add(vertices=10, radius=0.4, depth=0.3, location=(1.2, 1.1, -0.3), rotation=(1.5707963, 0, 0))
bpy.data.objects['Cylinder'].name = 'Trim'
bpy.data.materials.new('Chrome')
bpy.data.objects['Trim'].data.materials.append(bpy.data.materials['Chrome'])
bpy.data.materials['Chrome'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 1.0
bpy.data.materials['Chrome'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.05
bpy.data.materials['Chrome'].node_tree.nodes['Principled BSDF'].inputs['Coat Weight'].default_value = 1.0
bpy.ops.export_scene.gltf(filepath='out/car.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/car.glb')
bpy.ops.export_scene.gltf(filepath='out/car_rt.glb')
