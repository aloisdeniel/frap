# WP 3.9: a normal-mapped rock for a game engine that needs tangents. A smooth ico sphere with
# smart_project UVs and a normal map exports with export_tangents=True (MikkTSpace TANGENT, w the
# bitangent sign); the same file without tangents is the default. Z-up (export_yup=False)
# changes positions, normals and tangents. Round trip (§17.2) of the three files, each
# re-exported with its own options (the importer drops tangents, the exporter recomputes them;
# the Z-up file is read as if it were Y-up). tol=1e-4: smart_project puts one UV component of
# two corners 1.0e-4 from Blender's (listed in docs/deviations.md §2 with the smart_project gap).
# golden: startup=factory tier=T2 tol=1e-4 compare=glb:out/rock.glb,glb:out/rock_plain.glb,glb:out/rock_zup.glb,glb:out/rock_rt.glb,glb:out/rock_plain_rt.glb,glb:out/rock_zup_rt.glb
import bpy
bpy.data.objects.remove(bpy.data.objects['Cube'])
bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=2, radius=1.0)
bpy.context.object.name = 'Rock'
bpy.ops.object.shade_smooth()
bpy.data.objects['Rock'].data.materials.append(bpy.data.materials['Material'])
bpy.data.images.new('RockNormal', 2, 2)
bpy.data.images['RockNormal'].colorspace_settings.name = 'Non-Color'
bpy.data.images['RockNormal'].pixels = [0.5, 0.5, 1.0, 1.0, 0.7, 0.5, 0.8, 1.0, 0.5, 0.3, 0.8, 1.0, 0.4, 0.6, 0.9, 1.0]
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeTexImage')
bpy.data.materials['Material'].node_tree.nodes['Image Texture'].image = bpy.data.images['RockNormal']
bpy.data.materials['Material'].node_tree.nodes.new('ShaderNodeNormalMap')
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Image Texture'].outputs['Color'], bpy.data.materials['Material'].node_tree.nodes['Normal Map'].inputs['Color'])
bpy.data.materials['Material'].node_tree.links.new(bpy.data.materials['Material'].node_tree.nodes['Normal Map'].outputs['Normal'], bpy.data.materials['Material'].node_tree.nodes['Principled BSDF'].inputs['Normal'])
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.smart_project()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.export_scene.gltf(filepath='out/rock.glb', export_tangents=True)
bpy.ops.export_scene.gltf(filepath='out/rock_plain.glb')
bpy.ops.export_scene.gltf(filepath='out/rock_zup.glb', export_tangents=True, export_yup=False)
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/rock.glb')
bpy.ops.export_scene.gltf(filepath='out/rock_rt.glb', export_tangents=True)
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/rock_plain.glb')
bpy.ops.export_scene.gltf(filepath='out/rock_plain_rt.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/rock_zup.glb')
bpy.ops.export_scene.gltf(filepath='out/rock_zup_rt.glb', export_tangents=True, export_yup=False)
