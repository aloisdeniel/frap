# WP 2.4: mesh.select_face_by_sides(type='NOTEQUAL', extend=False) on a cylinder merged with a
# triangle-fan circle and an unfilled circle: everything but the quads (the wire circle is
# deselected too).
# golden: startup=factory compare=objects,context,mesh:Cylinder tier=T1
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=8)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.primitive_circle_add(vertices=6, fill_type='TRIFAN', location=(3, 0, 0))
bpy.ops.mesh.primitive_circle_add(vertices=5, location=(-3, 0, 0))
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.select_face_by_sides(number=4, type='NOTEQUAL', extend=False)
