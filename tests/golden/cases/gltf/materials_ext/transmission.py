# WP 3.6: KHR_materials_transmission export. A constant factor on the first material; on the
# second, an Image Texture (copied whole) behind a Math MULTIPLY by 0.5 (the factor); a third
# material with Transmission Weight 0 has no extension.
# golden: startup=factory tier=T2 compare=glb:out/transmission.glb glb_structure
import bpy
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Transmission Weight'].default_value = 0.6
bpy.data.materials.new('Frosted')
bpy.data.images.new('Frost', 4, 4)
bpy.data.materials['Frosted'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Frosted'].node_tree.nodes['Image Texture'].image = bpy.data.images['Frost']
bpy.data.materials['Frosted'].node_tree.nodes.new('ShaderNodeMath')
bpy.data.materials['Frosted'].node_tree.nodes['Math'].operation = 'MULTIPLY'
bpy.data.materials['Frosted'].node_tree.nodes['Math'].inputs[1].default_value = 0.5
bpy.data.materials['Frosted'].node_tree.links.new(bpy.data.materials['Frosted'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Frosted'].node_tree.nodes['Math'].inputs[0])
bpy.data.materials['Frosted'].node_tree.links.new(bpy.data.materials['Frosted'].node_tree.nodes['Math'].outputs[0], bpy.data.materials['Frosted'].node_tree.nodes['Principled BSDF'].inputs['Transmission Weight'])
bpy.data.materials.new('Opaque')
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.context.object.data.materials.append(bpy.data.materials['Frosted'])
bpy.ops.mesh.primitive_plane_add(location=(-3, 0, 0))
bpy.context.object.data.materials.append(bpy.data.materials['Opaque'])
bpy.ops.export_scene.gltf(filepath='out/transmission.glb')
