# WP 1.23: import of materials: base colour, metallic and roughness factors raised back into a
# Principled BSDF, two slots on one mesh.
# Each tool imports its own export, whose vertex order may legitimately differ
# (glTF outputs compare canonically), so meshes compare canonically.
# golden: order=canonical startup=factory tier=T1 compare=objects,materials,mesh:Cube tol=1e-5
import bpy
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (0.2, 0.5, 0.8, 1)
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 0.75
bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Roughness'].default_value = 0.2
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('Red'))
bpy.data.materials['Red'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (1, 0, 0, 1)
bpy.data.meshes['Cube'].polygons[0].material_index = 1
bpy.ops.export_scene.gltf(filepath='out/mat.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/mat.glb')
