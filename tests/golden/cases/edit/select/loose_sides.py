# WP 2.4: mesh.select_face_by_sides (LESS / EQUAL / GREATER / NOTEQUAL, extend) on a
# cylinder (8-gon caps, quad sides) merged with a triangle-fan circle and an unfilled circle,
# and mesh.select_loose: the circle's wire edges in edge mode, nothing in face mode (no face
# is isolated) then the loose face of a plane merged into the mesh.
# golden: startup=factory compare=objects,context,mesh:Cylinder tier=T1
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=8)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.primitive_circle_add(vertices=6, fill_type='TRIFAN', location=(3, 0, 0))
bpy.ops.mesh.primitive_circle_add(vertices=5, location=(-3, 0, 0))
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_face_by_sides()
bpy.data.meshes['Cylinder'].total_face_sel
bpy.ops.mesh.select_face_by_sides(number=3, type='EQUAL', extend=False)
bpy.data.meshes['Cylinder'].total_face_sel
bpy.ops.mesh.select_face_by_sides(number=4, type='GREATER', extend=False)
bpy.data.meshes['Cylinder'].total_face_sel
bpy.ops.mesh.select_face_by_sides(number=4, type='LESS')
bpy.ops.mesh.select_face_by_sides(number=4, type='NOTEQUAL', extend=False)
bpy.ops.mesh.select_loose()
bpy.data.meshes['Cylinder'].total_vert_sel
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.mesh.select_loose()
bpy.data.meshes['Cylinder'].total_edge_sel
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.mesh.select_loose()
bpy.ops.mesh.primitive_plane_add(size=1, location=(0, 3, 0))
bpy.ops.mesh.select_loose(extend=True)
bpy.data.meshes['Cylinder'].total_face_sel
