# WP 1.26: polls do not depend on the selection for the M1 object operators: with nothing
# selected `delete` and `select_all` still poll true (`delete` is then CANCELLED, which the
# line comparator sees as not ok on both sides), and every primitive polls true. Blender's
# values are asserted on `#@blender` lines.
# golden: startup=factory tier=T0 compare=objects,context
import bpy
bpy.ops.object.select_all(action='DESELECT')
bpy.ops.object.delete.poll()
#@blender assert bpy.ops.object.delete.poll() is True
bpy.ops.object.delete()
bpy.ops.object.select_all.poll()
#@blender assert bpy.ops.object.select_all.poll() is True
bpy.ops.mesh.primitive_cylinder_add.poll()
#@blender assert bpy.ops.mesh.primitive_cylinder_add.poll() is True
bpy.ops.mesh.primitive_cone_add.poll()
#@blender assert bpy.ops.mesh.primitive_cone_add.poll() is True
bpy.ops.mesh.primitive_torus_add.poll()
#@blender assert bpy.ops.mesh.primitive_torus_add.poll() is True
bpy.ops.mesh.primitive_grid_add.poll()
#@blender assert bpy.ops.mesh.primitive_grid_add.poll() is True
bpy.ops.mesh.primitive_ico_sphere_add.poll()
#@blender assert bpy.ops.mesh.primitive_ico_sphere_add.poll() is True
bpy.ops.mesh.primitive_uv_sphere_add.poll()
#@blender assert bpy.ops.mesh.primitive_uv_sphere_add.poll() is True
bpy.ops.mesh.primitive_circle_add.poll()
#@blender assert bpy.ops.mesh.primitive_circle_add.poll() is True
bpy.ops.mesh.primitive_torus_add(location=(3, 0, 0))
bpy.ops.object.delete()
