# WP 3.9: export options an agent tunes for a mobile viewer, on a two-material scene with a
# texture: export_materials='PLACEHOLDER' (primitives keep their split, materials become
# unnamed placeholders), export_normals=False, export_texcoords=False and export_yup=False
# (Z-up nodes). Round trip (§17.2) of the Z-up file (the importer assumes Y-up, so the
# re-export carries the extra rotation), the placeholder file and the file without normals.
# Left out: export_image_format='NONE' (frap reads the factors through the node patterns
# there, a listed M7 gap) and the round trip of the file without UVs (Blender re-exports the
# texture with `"texCoord": -1`, `uv_layers.active_render_index` of a mesh without UV maps;
# frap writes 0).
# golden: startup=factory tier=T2 compare=glb:out/placeholder.glb,glb:out/no_normals.glb,glb:out/no_uvs.glb,glb:out/zup.glb,glb:out/zup_rt.glb,glb:out/placeholder_rt.glb,glb:out/no_normals_rt.glb
import bpy
bpy.data.images.new('Grid', 2, 2)
bpy.data.images['Grid'].pixels = [1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0]
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['Grid']
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Base Color'])
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('Red'))
bpy.data.materials['Red'].node_tree.nodes['Principled BSDF'].inputs['Base Color'].default_value = (1.0, 0.0, 0.0, 1.0)
bpy.data.meshes['Cube'].polygons[0].material_index = 1
bpy.data.meshes['Cube'].polygons[3].material_index = 1
bpy.data.objects['Cube'].location = (1, 2, 3)
bpy.data.objects['Cube'].rotation_euler = (0.3, 0.5, 0.7)
bpy.ops.export_scene.gltf(filepath='out/placeholder.glb', export_materials='PLACEHOLDER')
bpy.ops.export_scene.gltf(filepath='out/no_normals.glb', export_normals=False)
bpy.ops.export_scene.gltf(filepath='out/no_uvs.glb', export_texcoords=False)
bpy.ops.export_scene.gltf(filepath='out/zup.glb', export_yup=False)
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/zup.glb')
bpy.ops.export_scene.gltf(filepath='out/zup_rt.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/placeholder.glb')
bpy.ops.export_scene.gltf(filepath='out/placeholder_rt.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/no_normals.glb')
bpy.ops.export_scene.gltf(filepath='out/no_normals_rt.glb')
