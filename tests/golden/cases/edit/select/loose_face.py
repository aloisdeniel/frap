# WP 2.4: mesh.select_loose in face mode: no face of the cylinder or the triangle fan is
# isolated (that call checks the outcome only: the plane added next replaces the selection); the loose face of a plane added into the mesh joins the selected cylinder face
# with extend.
# golden: startup=factory compare=objects,context,mesh:Cylinder tier=T1
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=8)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.primitive_circle_add(vertices=6, fill_type='TRIFAN', location=(3, 0, 0))
bpy.ops.mesh.primitive_circle_add(vertices=5, location=(-3, 0, 0))
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.mesh.select_loose()
bpy.ops.mesh.primitive_plane_add(size=1, location=(0, 3, 0))
bpy.ops.frap.select_elements(type='FACE', indices=[0])
bpy.ops.mesh.select_loose(extend=True)
