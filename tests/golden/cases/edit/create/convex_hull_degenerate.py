# WP 2.8: mesh.convex_hull on degenerate input, as Bullet's btConvexHullComputer gives it to
# Blender: a planar point set is a two-sided polygon (both sides fanned from their first
# vertex), three points a triangle. Flat 3x3 grid (interior and edge-midpoint vertices
# deleted), 8-vertex wire circle, three loose vertices.
# golden: startup=empty compare=objects,context,mesh:Grid,mesh:Circle,mesh:Tri tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=3, y_subdivisions=3, size=2)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.convex_hull(join_triangles=False)
bpy.data.meshes['Grid'].total_face_sel
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_circle_add(vertices=8, radius=1, fill_type='NOTHING', location=(3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.convex_hull(join_triangles=False)
bpy.data.meshes['Circle'].total_face_sel
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.select_all(action='DESELECT')
bpy.data.meshes.new('Tri').from_pydata([(0, 0, 0), (1, 0, 0), (0, 1, 0)], [], [])
bpy.data.objects.new('Tri', bpy.data.meshes['Tri'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Tri'])
bpy.context.view_layer.objects.active = bpy.data.objects['Tri']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.convex_hull(join_triangles=False)
bpy.data.meshes['Tri'].total_face_sel
