# WP 2.8 (needs 2.10 join_triangles): mesh.convex_hull with its defaults (join_triangles=True)
# on a cylinder and on a point cloud.
# golden: startup=empty compare=objects,context,mesh:Cylinder,mesh:Cloud tier=T1
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=10, radius=1, depth=2)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.convex_hull()
bpy.data.meshes['Cylinder'].total_face_sel
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.select_all(action='DESELECT')
bpy.data.meshes.new('Cloud').from_pydata([(0.1, 0.2, 0.05), (1.3, 0.1, -0.2), (0.4, 1.2, 0.3), (-0.9, 0.3, 0.1), (0.2, -1.1, 0.4), (0.3, 0.1, 1.4), (-0.2, 0.4, -1.3), (0.9, 0.8, 0.9), (-0.7, -0.6, -0.8), (1, 1, -1), (1, -1, -1), (-1, 1, 1)], [], [])
bpy.data.objects.new('Cloud', bpy.data.meshes['Cloud'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Cloud'])
bpy.context.view_layer.objects.active = bpy.data.objects['Cloud']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.convex_hull()
bpy.data.meshes['Cloud'].total_face_sel
