# WP 2.8: mesh.convex_hull where hull facets are coplanar (Bullet's planar n-gons fanned from
# their first vertex): a 12-vertex cylinder and a 3x3x3 point lattice (face, edge and
# centre points deleted), both with use_existing_faces=False, and a collinear chain (Bullet's hull
# is a two-vertex loop, which Blender skips: every input element ends up unused and deleted).
# golden: startup=empty compare=objects,context,mesh:Cylinder,mesh:Lattice,mesh:Chain tier=T1
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=12, radius=1, depth=2)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.convex_hull(join_triangles=False, use_existing_faces=False)
bpy.data.meshes['Cylinder'].total_face_sel
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.select_all(action='DESELECT')
bpy.data.meshes.new('Lattice').from_pydata([(3.5, -0.75, -1), (4, -0.75, -1), (4.5, -0.75, -1), (3.5, 0, -1), (4, 0, -1), (4.5, 0, -1), (3.5, 0.75, -1), (4, 0.75, -1), (4.5, 0.75, -1), (3.5, -0.75, 0), (4, -0.75, 0), (4.5, -0.75, 0), (3.5, 0, 0), (4, 0, 0), (4.5, 0, 0), (3.5, 0.75, 0), (4, 0.75, 0), (4.5, 0.75, 0), (3.5, -0.75, 1), (4, -0.75, 1), (4.5, -0.75, 1), (3.5, 0, 1), (4, 0, 1), (4.5, 0, 1), (3.5, 0.75, 1), (4, 0.75, 1), (4.5, 0.75, 1)], [], [])
bpy.data.objects.new('Lattice', bpy.data.meshes['Lattice'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Lattice'])
bpy.context.view_layer.objects.active = bpy.data.objects['Lattice']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.convex_hull(join_triangles=False, use_existing_faces=False)
bpy.data.meshes['Lattice'].total_face_sel
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.select_all(action='DESELECT')
bpy.data.meshes.new('Chain').from_pydata([(0, 0, 0), (1, 0, 0), (2, 0, 0), (3, 0, 0)], [(0, 1), (1, 2), (2, 3)], [])
bpy.data.objects.new('Chain', bpy.data.meshes['Chain'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Chain'])
bpy.context.view_layer.objects.active = bpy.data.objects['Chain']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.convex_hull(join_triangles=False)
bpy.data.meshes['Chain'].total_face_sel
