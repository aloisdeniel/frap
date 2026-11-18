# WP 2.4: mesh.select_face_by_sides on a cylinder merged with a triangle-fan circle and an
# unfilled circle: the defaults (EQUAL 4, extend) select the quads, then LESS than 4 adds the
# triangles.
# golden: startup=factory compare=objects,context,mesh:Cylinder tier=T1
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=8)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.primitive_circle_add(vertices=6, fill_type='TRIFAN', location=(3, 0, 0))
bpy.ops.mesh.primitive_circle_add(vertices=5, location=(-3, 0, 0))
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_face_by_sides()
bpy.ops.mesh.select_face_by_sides(number=4, type='LESS')
