# WP 3.7: point clouds. A mesh of loose vertices exported with use_mesh_vertices is a POINTS-only
# glTF mesh; import_point_as_pointcloud=True turns it into a POINTCLOUD object (point cloud
# named like the mesh, positions converted to Z-up, quaternion rotation mode), the default
# import into a mesh of loose vertices. Writing a point radius creates the radius attribute
# (other points keep 0.01); the point cloud then exports as POINTS without indices, with
# _RADIUS and positions as stored (no Y-up conversion, as the add-on does); without
# use_mesh_vertices its node has no mesh.
# golden: startup=empty tier=T2 compare=objects,mesh:Pts.001,glb:out/pc.glb,glb:out/pc_off.glb
import bpy
bpy.data.meshes.new('Pts')
bpy.data.meshes['Pts'].from_pydata([(0, 0, 0), (1, 2, 3), (-1, 0.5, 2)], [], [])
bpy.data.objects.new('Pts', bpy.data.meshes['Pts'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Pts'])
bpy.ops.export_scene.gltf(filepath='out/pts.glb', use_mesh_vertices=True)
bpy.ops.import_scene.gltf(filepath='out/pts.glb', import_point_as_pointcloud=True)
bpy.ops.import_scene.gltf(filepath='out/pts.glb')
bpy.data.objects.remove(bpy.data.objects['Pts'])
bpy.data.objects.remove(bpy.data.objects['Pts.002'])
bpy.data.pointclouds['Pts'].points[1].radius = 0.5
bpy.data.pointclouds['Pts'].points[2].co = (4.0, 5.0, 6.0)
bpy.ops.export_scene.gltf(filepath='out/pc.glb', use_mesh_vertices=True)
bpy.ops.export_scene.gltf(filepath='out/pc_off.glb')
