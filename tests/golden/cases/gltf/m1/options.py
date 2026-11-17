# WP 1.23: honoured options: no normals and no UVs with materials 'NONE' (one primitive per
# mesh), Z-up, and materials 'PLACEHOLDER'.
# golden: startup=factory tier=T2 compare=glb:out/bare.glb,glb:out/zup.glb,glb:out/placeholder.glb
import bpy
bpy.data.meshes['Cube'].materials.append(bpy.data.materials.new('Second'))
bpy.data.meshes['Cube'].polygons[0].material_index = 1
bpy.ops.export_scene.gltf(filepath='out/bare.glb', export_normals=False, export_texcoords=False, export_materials='NONE')
bpy.ops.export_scene.gltf(filepath='out/zup.glb', export_yup=False)
bpy.ops.export_scene.gltf(filepath='out/placeholder.glb', export_materials='PLACEHOLDER')
