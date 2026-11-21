# WP 3.9: plastic toys. KHR_materials_specular: a Specular IOR Level of 0.25 with a tint
# (specularFactor 0.5), a level of 0.9 whose excess scales specularColorFactor, and a specular
# tint texture of distinct pixels; KHR_materials_ior 1.4 on the first (written because the
# specular extension is present). Round trip (§17.2).
# golden: startup=factory tier=T2 compare=glb:out/toys.glb,glb:out/toys_rt.glb glb_structure
import bpy
bpy.data.objects['Cube'].name = 'Block'
bpy.data.materials['Material'].name = 'Matte'
bpy.data.materials['Matte'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.1, 0.3, 0.9, 1.0)
bpy.data.materials['Matte'].node_tree.nodes['Principled BSDF'].inputs['Specular IOR Level'].default_value = 0.25
bpy.data.materials['Matte'].node_tree.nodes['Principled BSDF'].inputs['Specular Tint'].default_value = (0.9, 0.8, 0.7, 1.0)
bpy.data.materials['Matte'].node_tree.nodes['Principled BSDF'].inputs['IOR'].default_value = 1.4
bpy.ops.mesh.primitive_uv_sphere_add(segments=10, ring_count=5, location=(3, 0, 0))
bpy.data.objects['Sphere'].name = 'Ball'
bpy.data.materials.new('Glossy')
bpy.data.objects['Ball'].data.materials.append(bpy.data.materials['Glossy'])
bpy.data.materials['Glossy'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.9, 0.8, 0.1, 1.0)
bpy.data.materials['Glossy'].node_tree.nodes['Principled BSDF'].inputs['Specular IOR Level'].default_value = 0.9
bpy.data.materials['Glossy'].node_tree.nodes['Principled BSDF'].inputs['Specular Tint'].default_value = (0.5, 0.75, 1.0, 1.0)
bpy.ops.mesh.primitive_cone_add(vertices=8, location=(-3, 0, 0))
bpy.data.objects['Cone'].name = 'Hat'
bpy.data.materials.new('Pearl')
bpy.data.objects['Hat'].data.materials.append(bpy.data.materials['Pearl'])
bpy.data.images.new('Sheenmap', 2, 2)
bpy.data.images['Sheenmap'].pixels = [1.0, 0.9, 0.8, 1.0, 0.8, 0.9, 1.0, 1.0, 0.9, 1.0, 0.8, 1.0, 0.7, 0.7, 0.7, 1.0]
bpy.data.materials['Pearl'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Pearl'].node_tree.nodes['Image Texture'].image = bpy.data.images['Sheenmap']
bpy.data.materials['Pearl'].node_tree.links.new(bpy.data.materials['Pearl'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Pearl'].node_tree.nodes['Principled BSDF'].inputs['Specular Tint'])
bpy.ops.export_scene.gltf(filepath='out/toys.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/toys.glb')
bpy.ops.export_scene.gltf(filepath='out/toys_rt.glb')
