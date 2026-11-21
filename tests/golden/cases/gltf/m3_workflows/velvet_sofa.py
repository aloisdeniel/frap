# WP 3.9: a velvet sofa. KHR_materials_sheen with a sheen colour texture of distinct pixels on
# Sheen Tint and a constant Sheen Roughness on the seat; a cushion with a constant tint and
# weight 0.5; the wooden legs have no sheen (Sheen Weight 0, tint ignored). Round trip (§17.2).
# golden: startup=factory tier=T2 compare=glb:out/sofa.glb,glb:out/sofa_rt.glb glb_structure
import bpy
bpy.data.objects['Cube'].name = 'Seat'
bpy.data.objects['Seat'].scale = (2.0, 1.0, 0.4)
bpy.data.materials['Material'].name = 'Velvet'
bpy.data.materials['Velvet'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.2, 0.05, 0.3, 1.0)
bpy.data.materials['Velvet'].node_tree.nodes['Principled BSDF'].inputs['Sheen Weight'].default_value = 1.0
bpy.data.materials['Velvet'].node_tree.nodes['Principled BSDF'].inputs['Sheen Roughness'].default_value = 0.3
bpy.data.images.new('Pile', 2, 2)
bpy.data.images['Pile'].pixels = [0.9, 0.7, 1.0, 1.0, 0.6, 0.4, 0.8, 1.0, 0.3, 0.2, 0.5, 1.0, 1.0, 1.0, 1.0, 1.0]
bpy.data.materials['Velvet'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Velvet'].node_tree.nodes['Image Texture'].image = bpy.data.images['Pile']
bpy.data.materials['Velvet'].node_tree.links.new(bpy.data.materials['Velvet'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Velvet'].node_tree.nodes['Principled BSDF'].inputs['Sheen Tint'])
bpy.ops.mesh.primitive_uv_sphere_add(segments=12, ring_count=6, radius=0.5, location=(1, 0, 1))
bpy.ops.object.shade_smooth()
bpy.data.objects['Sphere'].name = 'Cushion'
bpy.data.materials.new('Plush')
bpy.data.objects['Cushion'].data.materials.append(bpy.data.materials['Plush'])
bpy.data.materials['Plush'].node_tree.nodes['Principled BSDF'].inputs['Sheen Weight'].default_value = 0.5
bpy.data.materials['Plush'].node_tree.nodes['Principled BSDF'].inputs['Sheen Tint'].default_value = (1.0, 0.8, 0.6, 1.0)
bpy.ops.mesh.primitive_cylinder_add(vertices=6, radius=0.1, depth=0.4, location=(1.8, 0.8, -0.6))
bpy.data.objects['Cylinder'].name = 'Leg'
bpy.data.materials.new('Oak')
bpy.data.objects['Leg'].data.materials.append(bpy.data.materials['Oak'])
bpy.data.materials['Oak'].node_tree.nodes['Principled BSDF'].inputs['Sheen Tint'].default_value = (1.0, 0.0, 0.0, 1.0)
bpy.ops.export_scene.gltf(filepath='out/sofa.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/sofa.glb')
bpy.ops.export_scene.gltf(filepath='out/sofa_rt.glb')
