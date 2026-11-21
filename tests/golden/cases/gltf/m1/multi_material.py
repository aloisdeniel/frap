# WP 1.23: one mesh with three material slots (one primitive per used slot; a slot no face
# uses) and a second object sharing the first material.
# golden: startup=factory tier=T2 compare=glb:out/multi.glb glb_structure
import bpy
bpy.data.materials.new('Red')
bpy.data.materials['Red'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (1, 0, 0, 1)
bpy.data.materials.new('Blue')
bpy.data.materials['Blue'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0, 0, 1, 1)
bpy.data.materials.new('Unused')
bpy.data.meshes['Cube'].materials.append(bpy.data.materials['Red'])
bpy.data.meshes['Cube'].materials.append(bpy.data.materials['Blue'])
bpy.data.meshes['Cube'].materials.append(bpy.data.materials['Unused'])
bpy.data.meshes['Cube'].polygons[0].material_index = 1
bpy.data.meshes['Cube'].polygons[1].material_index = 1
bpy.data.meshes['Cube'].polygons[2].material_index = 2
bpy.ops.mesh.primitive_plane_add(location=(3, 0, 0))
bpy.context.object.data.materials.append(bpy.data.materials['Red'])
bpy.ops.export_scene.gltf(filepath='out/multi.glb')
