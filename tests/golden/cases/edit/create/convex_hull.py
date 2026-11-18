# WP 2.8: mesh.convex_hull (join_triangles=False: the triangle merge is WP 2.10's) around a
# cloud of points in general position (interior points deleted with delete_unused, kept
# without), and the INFO `Requires at least three vertices`.
# golden: startup=empty compare=objects,context,mesh:Cloud,mesh:Keep tier=T1
import bpy
bpy.data.meshes.new('Cloud').from_pydata([(0.1, 0.2, 0.05), (1.3, 0.1, -0.2), (0.4, 1.2, 0.3), (-0.9, 0.3, 0.1), (0.2, -1.1, 0.4), (0.3, 0.1, 1.4), (-0.2, 0.4, -1.3), (0.9, 0.8, 0.9), (-0.7, -0.6, -0.8), (0.05, 0.1, 0.2)], [], [])
bpy.data.objects.new('Cloud', bpy.data.meshes['Cloud'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Cloud'])
bpy.context.view_layer.objects.active = bpy.data.objects['Cloud']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 1])
bpy.ops.mesh.convex_hull(join_triangles=False)
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.convex_hull(join_triangles=False)
bpy.data.meshes['Cloud'].total_face_sel
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.select_all(action='DESELECT')
bpy.data.meshes.new('Keep').from_pydata([(0.1, 0.2, 0.05), (1.3, 0.1, -0.2), (0.4, 1.2, 0.3), (-0.9, 0.3, 0.1), (0.2, -1.1, 0.4), (0.3, 0.1, 1.4), (-0.2, 0.4, -1.3), (0.05, 0.1, 0.2)], [(0, 7)], [])
bpy.data.objects.new('Keep', bpy.data.meshes['Keep'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Keep'])
bpy.context.view_layer.objects.active = bpy.data.objects['Keep']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.convex_hull(join_triangles=False, delete_unused=False)
