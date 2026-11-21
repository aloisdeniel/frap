# WP 3.9: a product shot staged for a web viewer: a textured, bevelled product on a turntable
# (an empty parent with a Z rotation), a camera aimed at it, a key area light (kept as a node
# without a light, the add-on's AREA warning), a spot and a sun, and a backdrop with an unlit
# material; exported with export_cameras, export_lights and export_apply (the product's BEVEL
# modifier applied). Round trip (§17.2) with cameras and lights.
# golden: startup=factory tier=T2 compare=glb:out/stage.glb,glb:out/stage_rt.glb glb_structure
import bpy
bpy.ops.object.empty_add(location=(0, 0, 0), rotation=(0, 0, 0.4))
bpy.context.object.name = 'Turntable'
bpy.data.objects['Cube'].name = 'Product'
bpy.data.objects['Product'].location = (0, 0, 1)
bpy.data.objects['Product'].parent = bpy.data.objects['Turntable']
bpy.data.objects['Product'].modifiers.new('Bevel', 'BEVEL')
bpy.data.objects['Product'].modifiers['Bevel'].width = 0.1
bpy.data.images.new('Label', 2, 2)
bpy.data.images['Label'].pixels = [0.9, 0.1, 0.1, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.1, 0.1, 0.9, 1.0]
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['Label']
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.2
bpy.ops.mesh.primitive_plane_add(size=10, location=(0, 4, 0), rotation=(1.5707963, 0, 0))
bpy.context.object.name = 'Backdrop'
bpy.data.materials.new('Studio')
bpy.data.objects['Backdrop'].data.materials.append(bpy.data.materials['Studio'])
bpy.data.materials['Studio'].node_tree.nodes.new('ShaderNodeBackground')
bpy.data.materials['Studio'].node_tree.nodes['Background'].inputs['Color'].default_value = (0.9, 0.9, 0.92, 1.0)
bpy.data.materials['Studio'].node_tree.links.new(bpy.data.materials['Studio'].node_tree.nodes['Background'].outputs[0], bpy.data.materials['Studio'].node_tree.nodes['Material Output'].inputs['Surface'])
bpy.data.objects['Camera'].location = (0, -6, 2)
bpy.data.objects['Camera'].rotation_euler = (1.4, 0, 0)
bpy.data.lights['Light'].type = 'AREA'
bpy.data.lights['Light'].energy = 500
bpy.ops.object.light_add(type='SPOT', location=(-3, -3, 4), rotation=(0.7, 0, -0.8))
bpy.data.lights['Spot'].energy = 600
bpy.data.lights['Spot'].spot_size = 0.6
bpy.ops.object.light_add(type='SUN', location=(0, 0, 8), rotation=(0.2, 0.1, 0))
bpy.data.lights['Sun'].energy = 1.5
bpy.ops.export_scene.gltf(filepath='out/stage.glb', export_cameras=True, export_lights=True, export_apply=True)
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/stage.glb')
bpy.ops.export_scene.gltf(filepath='out/stage_rt.glb', export_cameras=True, export_lights=True)
