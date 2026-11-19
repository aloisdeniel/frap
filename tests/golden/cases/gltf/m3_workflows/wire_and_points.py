# WP 3.9: a survey scene mixing faces, loose edges and loose vertices. A ground quad with a
# fence of loose edges and a few marker vertices exports TRIANGLES only by default, a LINES
# primitive with use_mesh_edges, a POINTS primitive with use_mesh_vertices, and both together;
# a point colour reaches every primitive. Round trip (§17.2) of all four files, each
# re-exported with its own options: import turns LINES into loose edges and POINTS into loose
# vertices.
# golden: startup=empty tier=T2 compare=glb:out/survey.glb,glb:out/survey_edges.glb,glb:out/survey_points.glb,glb:out/survey_all.glb,glb:out/survey_rt.glb,glb:out/survey_plain_rt.glb,glb:out/survey_edges_rt.glb,glb:out/survey_points_rt.glb
import bpy
bpy.data.meshes.new('Survey')
bpy.data.meshes['Survey'].from_pydata([(0, 0, 0), (4, 0, 0), (4, 4, 0), (0, 4, 0), (0, 0, 1), (4, 0, 1), (4, 4, 1), (2, 2, 0.5), (1, 3, 0.2), (3, 1, 0.3)], [(0, 4), (1, 5), (2, 6), (4, 5), (5, 6)], [(0, 1, 2, 3)])
bpy.data.meshes['Survey'].color_attributes.new('Tag', 'FLOAT_COLOR', 'POINT')
bpy.data.meshes['Survey'].color_attributes['Tag'].data[4].color = (1.0, 0.0, 0.0, 1.0)
bpy.data.meshes['Survey'].color_attributes['Tag'].data[7].color = (0.0, 0.0, 1.0, 1.0)
bpy.data.materials.new('Ground')
bpy.data.meshes['Survey'].materials.append(bpy.data.materials['Ground'])
bpy.data.objects.new('Survey', bpy.data.meshes['Survey'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Survey'])
bpy.ops.export_scene.gltf(filepath='out/survey.glb')
bpy.ops.export_scene.gltf(filepath='out/survey_edges.glb', use_mesh_edges=True)
bpy.ops.export_scene.gltf(filepath='out/survey_points.glb', use_mesh_vertices=True)
bpy.ops.export_scene.gltf(filepath='out/survey_all.glb', use_mesh_edges=True, use_mesh_vertices=True)
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/survey_all.glb')
bpy.ops.export_scene.gltf(filepath='out/survey_rt.glb', use_mesh_edges=True, use_mesh_vertices=True)
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/survey.glb')
bpy.ops.export_scene.gltf(filepath='out/survey_plain_rt.glb')
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/survey_edges.glb')
bpy.ops.export_scene.gltf(filepath='out/survey_edges_rt.glb', use_mesh_edges=True)
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/survey_points.glb')
bpy.ops.export_scene.gltf(filepath='out/survey_points_rt.glb', use_mesh_vertices=True)
