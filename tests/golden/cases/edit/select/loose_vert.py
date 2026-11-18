# WP 2.4: mesh.select_loose in vertex mode without extend: the selection is cleared and no
# vertex is loose (every vertex has an edge), so nothing stays selected.
# golden: startup=factory compare=objects,context,mesh:Cylinder tier=T1
import bpy
bpy.ops.mesh.primitive_cylinder_add(vertices=8)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.primitive_circle_add(vertices=6, fill_type='TRIFAN', location=(3, 0, 0))
bpy.ops.mesh.primitive_circle_add(vertices=5, location=(-3, 0, 0))
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.select_loose()
