# WP 3.7: use_mesh_edges / use_mesh_vertices: a mesh with two faces, three loose edges and two
# loose vertices exports a TRIANGLES primitive, then one LINES primitive of the loose edges
# (material of slot 0) and one POINTS primitive of the loose vertices; a POINT colour reaches
# the lines and points, the fake white COLOR_0 too; off by default. A mesh made only of
# vertices exports a lone POINTS primitive without material. Each file is imported back:
# LINES become loose edges and POINTS loose vertices, equal to Blender.
# golden: startup=factory tier=T2 compare=glb:out/default.glb,glb:out/edges.glb,glb:out/points.glb,glb:out/both.glb,glb:out/cloud.glb,mesh:Loose.001,mesh:Loose.002,mesh:Cloud.001 order=canonical tol=1e-5 glb_structure
import bpy
bpy.data.meshes.new('Loose')
bpy.data.meshes['Loose'].from_pydata([(0, 0, 0), (1, 0, 0), (1, 1, 0), (0, 1, 0), (2, 0, 0), (3, 0, 1), (3, 2, 1), (4, 2, 0), (5, 5, 5), (-1, -2, 0.5), (2, 1, 0)], [(4, 5), (5, 6), (7, 6)], [(0, 1, 2, 3), (1, 4, 10, 2)])
bpy.data.meshes['Loose'].materials.append(bpy.data.materials['Material'])
bpy.data.meshes['Loose'].color_attributes.new('PC', 'FLOAT_COLOR', 'POINT')
bpy.data.meshes['Loose'].color_attributes['PC'].data[5].color = (1.0, 0.5, 0.0, 1.0)
bpy.data.meshes['Loose'].color_attributes['PC'].data[8].color = (0.0, 0.5, 1.0, 0.5)
bpy.data.objects.new('Loose', bpy.data.meshes['Loose'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Loose'])
bpy.data.objects.remove(bpy.data.objects['Cube'])
bpy.ops.export_scene.gltf(filepath='out/default.glb')
bpy.ops.export_scene.gltf(filepath='out/edges.glb', use_mesh_edges=True)
bpy.ops.export_scene.gltf(filepath='out/points.glb', use_mesh_vertices=True)
bpy.ops.export_scene.gltf(filepath='out/both.glb', use_mesh_edges=True, use_mesh_vertices=True, export_vertex_color='NONE')
bpy.data.objects.remove(bpy.data.objects['Loose'])
bpy.data.meshes.new('Cloud')
bpy.data.meshes['Cloud'].from_pydata([(0, 0, 0), (1, 2, 3), (-1, 0.5, 2), (4, 4, 4)], [], [])
bpy.data.objects.new('Cloud', bpy.data.meshes['Cloud'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Cloud'])
bpy.ops.export_scene.gltf(filepath='out/cloud.glb', use_mesh_vertices=True)
bpy.ops.import_scene.gltf(filepath='out/both.glb')
bpy.ops.import_scene.gltf(filepath='out/edges.glb')
bpy.ops.import_scene.gltf(filepath='out/cloud.glb')
