# WP 3.9: a neon sign. The tube emits orange at Emission Strength 6 (emissiveFactor and
# KHR_materials_emissive_strength), the backing board is unlit (a Background shader:
# KHR_materials_unlit) and a lamp uses an Emission shader alone (emissive only, black base
# colour). Round trip (§17.2).
# golden: startup=factory tier=T2 compare=glb:out/sign.glb,glb:out/sign_rt.glb
import bpy
bpy.ops.mesh.primitive_torus_add(major_segments=16, minor_segments=6, major_radius=1.0, minor_radius=0.1, location=(0, 0, 2), rotation=(1.5707963, 0, 0))
bpy.data.objects['Torus'].name = 'Tube'
bpy.data.materials.new('Neon')
bpy.data.objects['Tube'].data.materials.append(bpy.data.materials['Neon'])
bpy.data.materials['Neon'].node_tree.nodes['Principled BSDF'].inputs['Emission Color'].default_value = (1.0, 0.4, 0.05, 1.0)
bpy.data.materials['Neon'].node_tree.nodes['Principled BSDF'].inputs['Emission Strength'].default_value = 6.0
bpy.data.materials['Neon'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.1, 0.05, 0.0, 1.0)
bpy.data.objects['Cube'].name = 'Board'
bpy.data.objects['Board'].scale = (1.5, 0.1, 1.5)
bpy.data.objects['Board'].location = (0, 0.3, 2)
bpy.data.materials['Material'].name = 'Backing'
bpy.data.materials['Backing'].node_tree.nodes.new('ShaderNodeBackground')
bpy.data.materials['Backing'].node_tree.nodes['Background'].inputs['Color'].default_value = (0.05, 0.05, 0.2, 1.0)
bpy.data.materials['Backing'].node_tree.links.new(bpy.data.materials['Backing'].node_tree.nodes['Background'].outputs[0], bpy.data.materials['Backing'].node_tree.nodes['Material Output'].inputs['Surface'])
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=1, radius=0.3, location=(2, 0, 1))
bpy.data.objects['Icosphere'].name = 'Lamp'
bpy.data.materials.new('Bulb')
bpy.data.objects['Lamp'].data.materials.append(bpy.data.materials['Bulb'])
bpy.data.materials['Bulb'].node_tree.nodes.new('ShaderNodeEmission')
bpy.data.materials['Bulb'].node_tree.nodes['Emission'].inputs['Color'].default_value = (1.0, 0.9, 0.6, 1.0)
bpy.data.materials['Bulb'].node_tree.nodes['Emission'].inputs['Strength'].default_value = 2.5
bpy.data.materials['Bulb'].node_tree.links.new(bpy.data.materials['Bulb'].node_tree.nodes['Emission'].outputs[0], bpy.data.materials['Bulb'].node_tree.nodes['Material Output'].inputs['Surface'])
bpy.ops.export_scene.gltf(filepath='out/sign.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/sign.glb')
bpy.ops.export_scene.gltf(filepath='out/sign_rt.glb')
