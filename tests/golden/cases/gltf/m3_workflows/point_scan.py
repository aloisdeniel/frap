# WP 3.9: a scanned point set. A mesh made only of vertices exports as a lone POINTS primitive
# (use_mesh_vertices=True). Round trip (§17.2): imported as a mesh of loose vertices, its
# re-export matches the source. Imported as a POINTCLOUD object (import_point_as_pointcloud),
# the point cloud needs a radius attribute before it can be exported (the add-on's exporter
# raises KeyError 'radius' without one; frap refuses it too), so one radius is written; it then
# exports POINTS again with _RADIUS (0.01 for the other points) and the positions as stored.
# golden: startup=empty tier=T2 compare=glb:out/scan.glb,glb:out/scan_rt.glb,glb:out/scan_cloud_rt.glb
import bpy
bpy.data.meshes.new('Scan')
bpy.data.meshes['Scan'].from_pydata([(0, 0, 0), (1, 0.5, 0.2), (2, 1, 0.1), (0.5, 2, 1.5), (-1, -1, 0.75), (3, 3, 3)], [], [])
bpy.data.objects.new('Scan', bpy.data.meshes['Scan'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Scan'])
bpy.data.objects['Scan'].location = (1, 2, 0)
bpy.ops.export_scene.gltf(filepath='out/scan.glb', use_mesh_vertices=True)
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/scan.glb')
bpy.ops.export_scene.gltf(filepath='out/scan_rt.glb', use_mesh_vertices=True)
bpy.ops.wm.read_homefile(use_empty=True)
bpy.ops.import_scene.gltf(filepath='out/scan.glb', import_point_as_pointcloud=True)
bpy.data.pointclouds['Scan'].points[2].radius = 0.25
bpy.ops.export_scene.gltf(filepath='out/scan_cloud_rt.glb', use_mesh_vertices=True)
